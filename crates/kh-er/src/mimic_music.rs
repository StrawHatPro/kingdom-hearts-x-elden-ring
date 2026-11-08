//! (MIMIC-MUSIC) KH3'S "REPLICAS" IN PLACE OF ELDEN RING'S BOSS MUSIC, IN THE TWO MIMIC TEAR FIGHTS ONLY. THE USER (2026-11-05):
//! "for the mimic tear boss fights, replace the music that plays during the boss fight with the song "Replicas" from KH3".
//! State note: build/sora_tmp/weapons/MIMIC-BOSS/MUSIC.md.
//!
//! WHAT PLAYS: KH3's battle theme of the replica Xehanorts, `/Game/Sound/BGM/BGM_xeha_bat__boss_1` (music section "066_M",
//! the same master as Re Mind's `BGM_battle_kg06_repx`; SEAD music Volume 0.46), from the user's own install (the extra audio
//! root PACK/mimic/audio, tools/mimic_music_pack.js), on OUR output (kh-audio), looped at KH3's loop points (an 11.642 s
//! intro, then [558816, 13405092) of 48 kHz), at the settings' Music level (Master on the output x Music on its BGM
//! category: fx.rs `apply_levels`, TITLE-AUDIO's rule; Sound Effects never reaches it).
//!
//! WHEN: exactly when Elden Ring's own boss music would. Both fights start, change and stop their music through the common
//! event 9005822 (2.7.2.0's own scripts, MIMIC-BOSS/trace): `2010[10] SetBossBGM(921100, state)`, state 0 = the fight's
//! music starts (the fog passed), 1 = its phase 2, -1 = the defeat's stop (-2 = the other stop, unused by these two). Its
//! core, `boss_bgm(id, state)` (the EMEVD bank 2010 dispatcher's case 10, 0x5F24A0 / 0x5F2950), is detoured: every ask is
//! taken (and passed on: Elden Ring's sound state stays its own).
//! - THE GATE ([`gate`]): BGM 921100 is SHARED by many of Elden Ring's bosses (m11_05, m12_03, m61 ... 13 event files), so an
//!   ask counts only when the player stands in one of the two Mimic Tear maps (m12_02_00_00 / m30_20_00_00) and THAT boss's
//!   own flags (kh_partyhost's one shared list `MIMIC_BOSSES`) say its fight is on: its fog passed (the event's start
//!   condition), its defeat flag not set.
//! - START: the gated ask with state 0 (or 1). Elden Ring's music is silenced (below) and KH3's track starts.
//! - PHASE 2 (state 1): KH3's track has none; it plays on (logged).
//! - END: the defeat (state -1 / -2): KH3's track fades over `mimic.music_fade` (default 2.0 s: KH3's own BGM stop,
//!   `USQEXSEADStatics::StopAllBGM_OptionalFade` as KH3 calls it); Elden Ring's music is given back 6.0 s later
//!   (`mimic.er_restore_after`; its boss-BGM manager's own stop timer is 5.0 s, 0x1432A1964) over 1.0 s. The leader's death
//!   (`sora_frame(leader_down)`): the track fades over 2.0 s (KH3's game over calls that stop with 2.0 s, 0x140EAD210);
//!   Elden Ring's music stays silent until the load / respawn (Sora's frames stop) or the leader stands again. A reload,
//!   a quit or a warp (Sora's frames stop for 1 s): the track stops, Elden Ring's music is given back at once.
//!
//! ELDEN RING'S SIDE: ONLY ITS MUSIC is silenced (SFX, voices, ambience stay), through the game's own Music control: the sound
//! engine's "BgmVolume" RTPC setter (0xDC4F50 / 0xDC6640, the one its options apply calls for GameSettings + 0x04) is asked
//! for 0. The engine keeps the value it last sent at + 0x32C and the per-frame options apply sends again whenever the setting
//! differs from it, so + 0x32C is put back to the option's own value (the player's options are never touched or saved); a
//! slider change during the fight is seen (+ 0x32C moves) and silenced again. The restore asks the setter for that value.
//!
//! KEYS: `mimic.music = 0` (Elden Ring's own boss music in these fights), `mimic.music_fade` (s), `mimic.er_restore_after` (s).

use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU64, AtomicU8, AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

use crate::control::Control;
use crate::erlog;
use crate::havok::{locate, Located};

/// The KH3 asset that plays: the replica Xehanorts' battle theme (main game bank; Re Mind's `BGM_battle_kg06_repx` is the
/// same master).
pub const REPLICAS_ASSET: &str = "/Game/Sound/BGM/BGM_xeha_bat__boss_1";
/// Its loop (48 kHz frames, [start, end)): the bank's material loop fields.
pub const REPLICAS_LOOP: (u32, u32) = (558_816, 13_405_092);
pub const REPLICAS_RATE: u32 = 48_000;
/// Elden Ring's boss-BGM id both Mimic Tear fights ask for (9005822's second argument; shared with other bosses).
pub const ER_BOSS_BGM: i32 = 921_100;
/// KH3's own BGM stop fade (titlemusic.rs: `StopAllBGM_OptionalFade` as KH3's game over calls it).
pub const KH3_BGM_FADE: f32 = crate::titlemusic::KH3_BGM_FADE;
/// Elden Ring's boss-BGM manager's stop timer (0x1432A1964 = 5.0, read by its update's -2 state), + 1 s: when Elden Ring's
/// music is given back after a defeat.
pub const ER_RESTORE_AFTER: f32 = 6.0;

/// The first second of KH3's track is in its intro: the loop pass is told 1 s past each loop end.
pub fn loop_passes(elapsed: f64) -> u64 {
    let rate = f64::from(REPLICAS_RATE);
    let (start, end) = (f64::from(REPLICAS_LOOP.0) / rate, f64::from(REPLICAS_LOOP.1) / rate);
    let t = elapsed - 1.0;
    if t < end {
        0
    } else {
        1 + ((t - end) / (end - start)) as u64
    }
}

// ------------------------------------------------------------------------------------------ the gate (pure)

/// One fight the music belongs to (from the shared list `kh_partyhost::mimicboss::MIMIC_BOSSES`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FightRow {
    pub key: &'static str,
    pub name: &'static str,
    pub area: u8,
    pub block: u8,
    pub defeat_flag: u32,
    pub fog_flag: u32,
}

/// The two fights (the shared list; without the party feature there is no Mimic Tear boss of ours: none).
pub fn fights() -> &'static [FightRow] {
    static ROWS: OnceLock<Vec<FightRow>> = OnceLock::new();
    ROWS.get_or_init(|| {
        #[cfg(feature = "party")]
        {
            kh_partyhost::mimicboss::MIMIC_BOSSES.iter().map(|b| FightRow { key: b.key, name: b.name, area: b.area, block: b.block, defeat_flag: b.defeat_flag, fog_flag: b.fog_flag }).collect()
        }
        #[cfg(not(feature = "party"))]
        {
            Vec::new()
        }
    })
}

/// The player's map block (`BlockId` as a number: area << 24 | block << 16 | ...) is the fight's map.
pub fn in_map(block: i32, row: &FightRow) -> bool {
    let b = block as u32;
    block > 0 && (b >> 24) & 0xFF == u32::from(row.area) && (b >> 16) & 0xFF == u32::from(row.block)
}

/// THE GATE: Elden Ring's boss music `id` asked in map block `block` belongs to a Mimic Tear fight of `rows` when it is 921100,
/// the player is in that boss's map, its fog is passed and it is not defeated (its own flags; `flag` None = not readable).
pub fn gate<'a>(id: i32, block: i32, rows: &'a [FightRow], flag: &dyn Fn(u32) -> Option<bool>) -> Option<&'a FightRow> {
    if id != ER_BOSS_BGM {
        return None;
    }
    rows.iter().find(|r| in_map(block, r) && flag(r.defeat_flag) == Some(false) && flag(r.fog_flag) == Some(true))
}

// ------------------------------------------------------------------------------------------ the fight's music (pure)

/// One ask of Elden Ring's `SetBossBGM(id, state)`, and the fight the gate gave it when it was made.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ask {
    pub id: i32,
    pub state: i32,
    pub fight: Option<&'static str>,
}

/// How KH3's track ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Why {
    /// Elden Ring stopped its boss music: the boss is defeated (state -1 / -2, or its defeat flag).
    Defeat,
    /// The leader went down.
    Death,
    /// Sora's frames stopped (a load, a respawn, a quit, a warp).
    Left,
    /// `mimic.music = 0`, or the payload stops.
    Off,
}

impl Why {
    pub fn words(self) -> &'static str {
        match self {
            Why::Defeat => "the boss is defeated (Elden Ring stops its boss music)",
            Why::Death => "the leader is down (KH3's game over stops its BGM)",
            Why::Left => "Sora's frames stopped (a load, a respawn, a quit or a warp)",
            Why::Off => "mimic.music = 0 / the payload stops",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Phase {
    #[default]
    Off,
    Playing(&'static str),
    /// KH3's track has ended; Elden Ring's music is still silent until `restore_at` (ms tick; 0 = until the leader stands or
    /// Sora's frames stop).
    Ended(Why, u64),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Act {
    /// Elden Ring's music silenced and KH3's track started, for this fight.
    Start(&'static str),
    /// Elden Ring's phase 2 of the fight (KH3's track plays on).
    Phase2,
    /// KH3's track fades out.
    Stop(Why),
    /// Elden Ring's music is given back.
    Restore(Why),
}

/// What this frame says.
#[derive(Clone, Copy, Debug, Default)]
pub struct Input {
    /// The feature is on and KH3's track is in the pack.
    pub on: bool,
    /// The leader is down (Sora's frame: a game over is under way).
    pub leader_down: bool,
    /// Sora's frames run (one within the last second).
    pub sora_live: bool,
    /// The playing fight's defeat flag is set.
    pub defeated: bool,
    pub now: u64,
}

/// THE RULE: started by the gated ask, ended where Elden Ring's own boss music ends.
#[derive(Clone, Copy, Debug, Default)]
pub struct Machine {
    pub phase: Phase,
}

impl Machine {
    pub fn frame(&mut self, asks: &[Ask], i: Input, restore_after_ms: u64) -> Vec<Act> {
        let mut acts = Vec::new();
        match self.phase {
            Phase::Playing(_) if !i.on => {
                acts.extend([Act::Stop(Why::Off), Act::Restore(Why::Off)]);
                self.phase = Phase::Off;
            }
            Phase::Playing(_) if !i.sora_live => {
                acts.extend([Act::Stop(Why::Left), Act::Restore(Why::Left)]);
                self.phase = Phase::Off;
            }
            Phase::Playing(_) => {
                let stop = asks.iter().any(|a| a.id == ER_BOSS_BGM && a.state < 0) || i.defeated;
                if stop {
                    acts.push(Act::Stop(Why::Defeat));
                    self.phase = Phase::Ended(Why::Defeat, i.now + restore_after_ms);
                } else if i.leader_down {
                    acts.push(Act::Stop(Why::Death));
                    self.phase = Phase::Ended(Why::Death, 0);
                } else if asks.iter().any(|a| a.id == ER_BOSS_BGM && a.state == 1) {
                    acts.push(Act::Phase2);
                }
            }
            Phase::Ended(why, at) => {
                let due = match why {
                    Why::Death => !i.sora_live || !i.leader_down,
                    _ => !i.sora_live || i.now >= at,
                };
                if due || !i.on {
                    acts.push(Act::Restore(why));
                    self.phase = Phase::Off;
                }
            }
            Phase::Off => {}
        }
        if self.phase == Phase::Off && i.on && i.sora_live && !i.leader_down {
            if let Some(fight) = asks.iter().filter(|a| a.id == ER_BOSS_BGM && (a.state == 0 || a.state == 1)).find_map(|a| a.fight) {
                acts.push(Act::Start(fight));
                self.phase = Phase::Playing(fight);
            }
        }
        acts
    }
}

// ------------------------------------------------------------------------------------------ the game's places

/// The EMEVD bank 2010 dispatcher's cases 9 (one argument) and 10 (`SetBossBGM`: `mov edx, [rax+4]; mov ecx, [rax]; call`):
/// capture 1 = `boss_bgm(id, state)` (CSSound + 0x328's boss-BGM manager's request, 0xDB4C90 / 0xDB6380, which stores the
/// state at + 0x1F4 and the Wwise value of the id's BgmBossChrIdConv text at + 0x1F8 for the sound update to play).
const PAT_BOSS_BGM: &str = "48 8b 52 10 e8 ? ? ? ? 8b 08 e8 ? ? ? ? e9 ? ? ? ? 49 8b 80 d8 00 00 00 48 85 c0 75 17 49 8b 90 d0 00 00 00 49 8b 88 c8 00 00 00 48 8b 52 10 e8 ? ? ? ? 8b 50 04 8b 08 e8 $ { ' } e9";
/// The sound engine's options apply (0xDC66B0 / 0xDC7DA0, every frame): `ucomiss xmm1, [rbx+0x32C]` (Music / 10 against the
/// value last sent) ... `call <the BgmVolume setter>` (capture 1).
const PAT_BGM_VOLUME: &str = "0f 2e 8b 2c 03 00 00 f3 44 0f 10 00 7a 02 74 0b 0f 28 d6 48 8b cb e8 $ { ' }";

pub(crate) const LOCATED: [Located; 2] = [
    Located { name: "EMEVD SetBossBGM core boss_bgm(id, state) (MIMIC-MUSIC)", pattern: PAT_BOSS_BGM, capture: 1, rva_2_7_1_0: 0x005f_24a0, rva_2_7_2_0: 0x005f_2950 },
    Located { name: "sound engine BgmVolume RTPC setter (MIMIC-MUSIC)", pattern: PAT_BGM_VOLUME, capture: 1, rva_2_7_1_0: 0x00dc_4f50, rva_2_7_2_0: 0x00dc_6640 },
];

/// The sound engine's value of the BgmVolume RTPC last sent (the options apply compares the Music setting with it).
const ENGINE_BGM: usize = 0x32C;

type BossBgmFn = unsafe extern "C" fn(i32, i32);
type BgmSetter = unsafe extern "C" fn(*mut core::ffi::c_void, f32, f32);

static ORIG_BOSS_BGM: AtomicUsize = AtomicUsize::new(0);
/// The asks of the game since the last frame.
static ASKS: Mutex<Vec<Ask>> = Mutex::new(Vec::new());
/// The feature's state for the hook: 0 = off / not known yet, 1 = on (the key on and KH3's track in the pack).
static ON: AtomicU8 = AtomicU8::new(0);
/// The player's map block as last read on the game thread (FRAME_BEGIN).
static BLOCK: AtomicI32 = AtomicI32::new(0);
/// Sora's frame: its last tick (ms) and whether the leader is down.
static SORA_AT: AtomicU64 = AtomicU64::new(0);
static LEADER_DOWN: AtomicBool = AtomicBool::new(false);

fn now() -> u64 {
    // SAFETY: plain getter.
    unsafe { windows_sys::Win32::System::SystemInformation::GetTickCount64() }
}

#[derive(Clone, Copy)]
struct Places {
    engine_static: usize,
    setter: usize,
}

fn places() -> Option<Places> {
    static PLACES: OnceLock<Option<Places>> = OnceLock::new();
    *PLACES.get_or_init(|| {
        let p = Places { engine_static: locate(&crate::gosound::LOCATED[2])?, setter: locate(&LOCATED[1])? };
        erlog!("mimic_music: (MIMIC-MUSIC) Elden Ring's BgmVolume setter and the sound engine located");
        Some(p)
    })
}

fn read_ptr(at: usize) -> usize {
    // SAFETY: `at` is a static of the running image (located by its pattern); a plain pointer word.
    unsafe { std::ptr::read_volatile(at as *const usize) }
}

fn read_f32(at: usize) -> f32 {
    // SAFETY: a float field of the live sound engine whose pointer was checked by the caller.
    unsafe { std::ptr::read_volatile(at as *const f32) }
}

/// Elden Ring's music silenced: the engine, and the option's own value kept at + 0x32C.
#[derive(Clone, Copy)]
struct ErMute {
    engine: usize,
    option: f32,
}

static MUTE: Mutex<Option<ErMute>> = Mutex::new(None);

/// Sends `value` through the game's own BgmVolume setter (`fade` s), then puts + 0x32C back to `keep` (the option's value: the
/// options apply sees nothing to send). Game thread only.
fn send(p: Places, engine: usize, value: f32, fade: f32, keep: Option<f32>) {
    // SAFETY: game thread (FRAME_BEGIN / the EMEVD step); the engine is the game's own singleton (pointer checked); the setter
    // is the game's own (its place checked for this version by its pattern) and takes (engine, value 0..1, fade seconds).
    unsafe {
        std::mem::transmute::<usize, BgmSetter>(p.setter)(engine as *mut core::ffi::c_void, value, fade);
        if let Some(keep) = keep {
            std::ptr::write_volatile((engine + ENGINE_BGM) as *mut f32, keep);
        }
    }
}

/// Elden Ring's music silenced (once; again when the game sent its option anew). The words for the log; None = nothing done.
fn er_mute(mute: &mut Option<ErMute>, fade: f32) -> Option<String> {
    let p = places()?;
    let engine = read_ptr(p.engine_static);
    if !crate::game::plausible(engine) {
        return Some("the sound engine is not made: Elden Ring's music is NOT silenced".to_string());
    }
    let sent = read_f32(engine + ENGINE_BGM);
    match *mute {
        Some(m) if m.engine == engine && (sent - m.option).abs() < 1e-6 => None,
        Some(m) if m.engine == engine => {
            send(p, engine, 0.0, 0.0, Some(sent));
            *mute = Some(ErMute { engine, option: sent });
            Some(format!("Elden Ring sent its Music option anew ({:.2} -> {sent:.2}: the slider moved): silenced again", m.option))
        }
        _ => {
            send(p, engine, 0.0, fade, Some(sent));
            *mute = Some(ErMute { engine, option: sent });
            Some(format!("Elden Ring's MUSIC silenced over {fade:.2} s by its own Music control (the BgmVolume RTPC setter sent 0; its option's value {sent:.2} kept at the engine's + 0x32C); its SFX, voices and ambience stay"))
        }
    }
}

fn er_restore(mute: &mut Option<ErMute>, fade: f32) -> String {
    let Some(m) = mute.take() else { return "Elden Ring's music was not silenced".to_string() };
    let Some(p) = places() else { return "not located".to_string() };
    let engine = read_ptr(p.engine_static);
    if engine != m.engine || !crate::game::plausible(engine) {
        return "the sound engine was made anew: nothing to give back".to_string();
    }
    send(p, engine, m.option, fade, None);
    format!("Elden Ring's MUSIC given back over {fade:.2} s (the BgmVolume RTPC setter sent its option's {:.2})", m.option)
}

/// The gate as the game reads now: the player's block (last FRAME_BEGIN) and the bosses' own flags.
fn gate_now(id: i32) -> Option<&'static FightRow> {
    gate(id, BLOCK.load(Ordering::Relaxed), fights(), &crate::bossreward::game_flag)
}

unsafe extern "C" fn boss_bgm_hook(id: i32, state: i32) {
    let original: BossBgmFn = std::mem::transmute(ORIG_BOSS_BGM.load(Ordering::Relaxed));
    let fight = gate_now(id).map(|f| f.key);
    if let Ok(mut asks) = ASKS.try_lock() {
        if asks.len() < 32 {
            asks.push(Ask { id, state, fight });
        }
    }
    // the gated start: Elden Ring's music silenced before the game's sound update plays it (no onset of it is heard)
    if fight.is_some() && (state == 0 || state == 1) && ON.load(Ordering::Relaxed) == 1 {
        if let Ok(mut mute) = MUTE.try_lock() {
            let _ = er_mute(&mut mute, 0.0);
        }
    }
    original(id, state)
}

/// Detours the game's SetBossBGM core (once per payload; undone by the renderer's shutdown with every other inline hook).
pub fn install() {
    // (the engine and the setter found now, not inside the hook's first call)
    if places().is_none() {
        erlog!("mimic_music: Elden Ring's BgmVolume setter / sound engine are not located: its music cannot be silenced, the Mimic Tear fights keep it");
        return;
    }
    let Some(at) = locate(&LOCATED[0]) else {
        erlog!("mimic_music: Elden Ring's SetBossBGM core is not located: the Mimic Tear fights keep Elden Ring's own music");
        return;
    };
    // SAFETY: the function's start at its located address (pattern + recorded RVA; `sub rsp, 38h; xor eax, eax`); our hook
    // has its signature (ecx = the BGM id, edx = the state; no return value).
    match unsafe { kh_er_render::inline_hook(at, boss_bgm_hook as *const () as usize, &ORIG_BOSS_BGM, "SetBossBGM core") } {
        Ok(()) => erlog!("mimic_music: (MIMIC-MUSIC) Elden Ring's SetBossBGM core is detoured (the Mimic Tear fights' music -> KH3's Replicas)"),
        Err(e) => {
            ORIG_BOSS_BGM.store(0, Ordering::SeqCst);
            erlog!("mimic_music: Elden Ring's SetBossBGM core was NOT detoured ({e})");
        }
    }
}

pub fn uninstalled() {
    ORIG_BOSS_BGM.store(0, Ordering::SeqCst);
}

// ------------------------------------------------------------------------------------------ the frame

struct State {
    machine: Machine,
    started: Option<Instant>,
    loops_told: u64,
    /// The track's fade still running (the engine is pumped until it is over).
    stopping_until: Option<Instant>,
    /// KH3's track in the pack: None = not known yet.
    asset: Option<bool>,
    preloaded: bool,
    told_off: bool,
    /// (MIMIC-BOSS hotfix 2) the second of play last probed.
    probe_told: u64,
}

static STATE: Mutex<State> = Mutex::new(State { machine: Machine { phase: Phase::Off }, started: None, loops_told: 0, stopping_until: None, asset: None, preloaded: false, told_off: false, probe_told: u64::MAX });

/// (sora.rs, every Sora frame) Sora's frames run; `leader_down`: the leader is down (a game over is under way).
/// (MIMIC-BOSS hotfix 2, the user: "everything needs to start as soon as you enter the fog gate") the fight began (mimic_boss.rs
/// at the fog gate): KH3's track starts now, as if Elden Ring had asked its boss music (it asks only with its boss bar, seconds
/// later; that ask then finds the track playing). Taken at the next frame.
static OUR_START: Mutex<Option<&'static str>> = Mutex::new(None);

/// (MIMIC-BOSS hotfix 2) THE FIGHT BEGINS: KH3's Replicas starts for the fight `key` (a row of `fights()`; the party build's
/// mimic_boss.rs calls it).
#[cfg(feature = "party")]
pub fn fight_begins(key: &'static str) {
    if let Ok(mut s) = OUR_START.lock() {
        *s = Some(key);
    }
}

/// (MIMIC-BOSS hotfix 2) The start the fight asked, as Elden Ring's own ask would be (its state 0), for the machine.
pub fn our_start_ask(key: Option<&'static str>) -> Option<Ask> {
    let key = key?;
    fights().iter().any(|f| f.key == key).then_some(Ask { id: ER_BOSS_BGM, state: 0, fight: Some(key) })
}

pub fn sora_frame(leader_down: bool) {
    SORA_AT.store(now(), Ordering::Relaxed);
    LEADER_DOWN.store(leader_down, Ordering::Relaxed);
}

fn seconds(control: &Control, key: &str, default: f32) -> f32 {
    control.f32(key).filter(|v| v.is_finite() && (0.0..=30.0).contains(v)).unwrap_or(default)
}

/// (frame.rs, FRAME_BEGIN, every frame: loading screens too)
pub fn frame(control: &Control, dt: f32) {
    let Ok(mut state) = STATE.lock() else { return };
    let t = now();
    // the player's block for the hook's gate (the game thread, a task phase)
    let block: i32 = crate::game::player_mut().map_or(0, |p| p.current_block_id.into());
    BLOCK.store(block, Ordering::Relaxed);
    let key_on = control.flag("mimic.music") != Some(false);
    if state.asset.is_none() {
        if let Some(found) = crate::fx::has_asset(REPLICAS_ASSET) {
            state.asset = Some(found);
            erlog!("mimic_music: KH3's Replicas ({REPLICAS_ASSET}) is {}", if found { "in the pack (PACK/mimic/audio): it replaces Elden Ring's boss music in the two Mimic Tear fights" } else { "NOT in the pack (PACK/mimic/audio, tools/mimic_music_pack.js): Elden Ring's own boss music plays there" });
        }
    }
    // read ahead (53 MB) once the player is in one of the two maps
    if !state.preloaded && state.asset == Some(true) && fights().iter().any(|f| in_map(block, f)) && crate::fx::preload_asset(REPLICAS_ASSET).is_some() {
        state.preloaded = true;
        erlog!("mimic_music: the player is in a Mimic Tear map (block {block:#010x}): KH3's Replicas read ahead");
    }
    let on = key_on && state.asset == Some(true);
    ON.store(u8::from(on), Ordering::Relaxed);
    if !key_on && !state.told_off {
        state.told_off = true;
        erlog!("mimic_music: mimic.music = 0: Elden Ring's own boss music in the Mimic Tear fights");
    }
    let mut asks: Vec<Ask> = ASKS.try_lock().map(|mut a| std::mem::take(&mut *a)).unwrap_or_default();
    // (MIMIC-BOSS hotfix 2) the fight's own start at the fog gate
    if let Some(ask) = our_start_ask(OUR_START.try_lock().ok().and_then(|mut s| s.take())) {
        erlog!("mimic_music: THE FIGHT BEGAN AT THE FOG GATE ({}): KH3's Replicas starts now (Elden Ring asks its boss music only with its boss bar)", ask.fight.unwrap_or("?"));
        asks.push(ask);
    }
    for a in &asks {
        match a.fight.and_then(|k| fights().iter().find(|f| f.key == k)) {
            Some(f) => erlog!("mimic_music: Elden Ring asks its boss music {} state {} (SetBossBGM) in the {} fight (block {block:#010x}, its fog flag {} on, its defeat flag {} off)", a.id, a.state, f.name, f.fog_flag, f.defeat_flag),
            None if a.id == ER_BOSS_BGM => erlog!("mimic_music: Elden Ring asks its boss music {} state {} (SetBossBGM) for another boss (block {block:#010x}): not a Mimic Tear fight, Elden Ring's own music", a.id, a.state),
            None => erlog!("mimic_music: Elden Ring asks its boss music {} state {} (SetBossBGM): not a Mimic Tear fight's", a.id, a.state),
        }
    }
    let defeated = match state.machine.phase {
        Phase::Playing(k) => fights().iter().find(|f| f.key == k).is_some_and(|f| crate::bossreward::game_flag(f.defeat_flag) == Some(true)),
        _ => false,
    };
    let sora_at = SORA_AT.load(Ordering::Relaxed);
    let input = Input { on, leader_down: LEADER_DOWN.load(Ordering::Relaxed), sora_live: sora_at != 0 && t.saturating_sub(sora_at) < 1000, defeated, now: t };
    let restore_after = (seconds(control, "mimic.er_restore_after", ER_RESTORE_AFTER) * 1000.0) as u64;
    let fade = seconds(control, "mimic.music_fade", KH3_BGM_FADE);
    let fade_in = seconds(control, "mimic.music_fade_in", fade_in()); // (MIMIC-BOSS hotfix 3) its fade-in (sheet music.fade_in)
    let vol_db = control.f32("snd.vol").unwrap_or(-6.0);
    for act in state.machine.frame(&asks, input, restore_after) {
        match act {
            Act::Start(key) => {
                let er = MUTE.lock().ok().and_then(|mut m| er_mute(&mut m, 0.0)).unwrap_or_else(|| "Elden Ring's music was already silenced at its ask".to_string());
                state.started = None;
                state.loops_told = 0;
                state.stopping_until = None;
                let name = fights().iter().find(|f| f.key == key).map_or(key, |f| f.name);
                erlog!("mimic_music: THE {name} FIGHT'S MUSIC BEGINS: KH3's Replicas ({REPLICAS_ASSET}) is asked in place of Elden Ring's boss music; {er}");
            }
            Act::Phase2 => erlog!("mimic_music: Elden Ring's phase 2 of the fight (SetBossBGM state 1): KH3's Replicas plays on (it has no phase of its own)"),
            Act::Stop(why) => {
                let fade_out = match why {
                    Why::Defeat => fade,
                    Why::Death => KH3_BGM_FADE,
                    Why::Left | Why::Off => 0.5,
                };
                let played = state.started.map_or(0.0, |s| s.elapsed().as_secs_f64());
                let _ = crate::fx::boss_music(REPLICAS_ASSET, dt, vol_db, None, fade_out);
                state.stopping_until = Some(Instant::now() + std::time::Duration::from_secs_f32(fade_out + 0.5));
                erlog!("mimic_music: THE FIGHT'S MUSIC ENDS: {}: KH3's Replicas fades out over {fade_out:.2} s after {played:.1} s of play", why.words());
            }
            Act::Restore(why) => {
                let fade_back = match why {
                    Why::Defeat => 1.0,
                    Why::Off => 0.5,
                    Why::Death | Why::Left => 0.0,
                };
                let er = MUTE.lock().map(|mut m| er_restore(&mut m, fade_back)).unwrap_or_default();
                erlog!("mimic_music: {er} ({})", why.words());
            }
        }
    }
    match state.machine.phase {
        Phase::Playing(_) => {
            if let Ok(mut m) = MUTE.try_lock() {
                if let Some(words) = er_mute(&mut m, 0.0) {
                    erlog!("mimic_music: {words}");
                }
            }
            if crate::fx::boss_music(REPLICAS_ASSET, dt, vol_db, Some(1.0), fade_in) == Some(true) {
                let started = *state.started.get_or_insert_with(Instant::now);
                // (MIMIC-BOSS hotfix 2, the user: "I wasn't hearing the music we chose playing") its real level once a second
                let whole = started.elapsed().as_secs();
                if whole != state.probe_told {
                    state.probe_told = whole;
                    if let Some(p) = crate::fx::boss_music_probe(REPLICAS_ASSET) {
                        erlog!("mimic_music: KH3's Replicas at {whole} s: {p}");
                    }
                }
                let passes = loop_passes(started.elapsed().as_secs_f64());
                if state.loops_told == 0 {
                    state.loops_told = 1;
                    let rows = crate::eroptions::sound_rows();
                    erlog!("mimic_music: KH3's Replicas PLAYS (looped at KH3's loop points [{}, {}) of 48 kHz: an {:.3} s intro, then {:.3} s loops; Master {} x Music {} on the BGM category, Sound Effects {} never reaches it)", REPLICAS_LOOP.0, REPLICAS_LOOP.1, f64::from(REPLICAS_LOOP.0) / 48_000.0, f64::from(REPLICAS_LOOP.1 - REPLICAS_LOOP.0) / 48_000.0, rows.master, rows.music, rows.sfx);
                } else if passes + 1 > state.loops_told {
                    state.loops_told = passes + 1;
                    erlog!("mimic_music: KH3's Replicas LOOPED at its loop end ({:.1} s since its start: pass {}), still playing", started.elapsed().as_secs_f64(), passes + 1);
                }
            }
        }
        _ => {
            if state.stopping_until.is_some_and(|until| Instant::now() < until) {
                let _ = crate::fx::boss_music(REPLICAS_ASSET, dt, vol_db, None, fade);
            } else if state.stopping_until.take().is_some() {
                let live = crate::fx::boss_music(REPLICAS_ASSET, dt, vol_db, None, fade) == Some(true);
                erlog!("mimic_music: KH3's Replicas has stopped ({})", if live { "STILL LIVE" } else { "no voice left" });
            }
        }
    }
}

/// (MIMIC-BOSS hotfix 3, THE USER 2026-11-06: "The music played correctly but it needs to fade in") Replicas' fade-in (s): the
/// row `music.fade_in` of `sheets/port/mimic-boss.json` (read once), else KH3's BGM fade [`KH3_BGM_FADE`] (no battle-BGM
/// fade-in time of KH3's is known to this port: the same 2.0 s as its stop).
fn fade_in() -> f32 {
    static X: std::sync::OnceLock<f32> = std::sync::OnceLock::new();
    *X.get_or_init(|| {
        let path = crate::log::dir().parent().map(|root| crate::sora::data_paths(root).1.with_file_name("mimic-boss.json"));
        let row = path.as_ref().and_then(|p| std::fs::read_to_string(p).ok()).and_then(|t| sheet_row(&t, "music.fade_in"));
        erlog!("mimic_music: KH3's Replicas fades in over {:.2} s ({})", row.unwrap_or(KH3_BGM_FADE), if row.is_some() { "sheet mimic-boss.json music.fade_in" } else { "KH3's BGM fade: no sheet row" });
        row.unwrap_or(KH3_BGM_FADE)
    })
}

/// A number row of a port sheet (`{"rows": [[id, value, ...], ..]}`); 0 is a value (no fade).
fn sheet_row(text: &str, id: &str) -> Option<f32> {
    let v: serde_json::Value = serde_json::from_str(text.trim_start_matches('﻿')).ok()?;
    v["rows"].as_array()?.iter().find(|r| r[0].as_str() == Some(id)).and_then(|r| r[1].as_f64()).map(|x| x as f32).filter(|x| x.is_finite() && *x >= 0.0)
}

/// (entry.rs, the payload's stop) Elden Ring's music is given back at once, KH3's track stops.
pub fn shutdown() {
    ON.store(0, Ordering::SeqCst);
    let Ok(mut state) = STATE.lock() else { return };
    if let Ok(mut m) = MUTE.lock() {
        if m.is_some() {
            let er = er_restore(&mut m, 0.0);
            erlog!("mimic_music: the payload stops: {er}");
        }
    }
    state.machine = Machine::default();
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOKRON: i32 = 0x0C02_0000; // m12_02_00_00
    const HALIGTREE_PATH: i32 = 0x1E14_0000; // m30_20_00_00

    #[cfg(feature = "party")] // (MIMIC-BOSS) only the party build's gate test reads it
    fn flags(on: &'static [u32]) -> impl Fn(u32) -> Option<bool> {
        move |f| Some(on.contains(&f))
    }

    fn ask(state: i32, fight: Option<&'static str>) -> Ask {
        Ask { id: ER_BOSS_BGM, state, fight }
    }

    fn live(now: u64) -> Input {
        Input { on: true, leader_down: false, sora_live: true, defeated: false, now }
    }

    /// (MIMIC-MUSIC) the gate: BGM 921100 is shared by other bosses of Elden Ring (2.7.2.0's own event files), so only the two
    /// Mimic Tear fights' maps with THAT boss's flags (fog passed, not defeated) count
    #[cfg(feature = "party")]
    #[test]
    fn mm_the_gate_covers_only_the_two_mimic_tear_fights() {
        let rows = fights();
        assert_eq!(rows.len(), 2);
        let (mimic, stray) = (rows.iter().find(|r| r.key == "mimic_tear").unwrap(), rows.iter().find(|r| r.key == "stray_mimic_tear").unwrap());
        assert_eq!((mimic.defeat_flag, mimic.fog_flag), (12_020_850, 12_022_855));
        assert_eq!((stray.defeat_flag, stray.fog_flag), (30_200_800, 30_202_805));
        // in the fight: fog passed, not defeated
        assert_eq!(gate(ER_BOSS_BGM, NOKRON, rows, &flags(&[12_022_855])).map(|r| r.key), Some("mimic_tear"));
        assert_eq!(gate(ER_BOSS_BGM, HALIGTREE_PATH | 0x0100, rows, &flags(&[30_202_805])).map(|r| r.key), Some("stray_mimic_tear"));
        // the fog not passed / the boss defeated / flags not readable: no
        assert!(gate(ER_BOSS_BGM, NOKRON, rows, &flags(&[])).is_none());
        assert!(gate(ER_BOSS_BGM, NOKRON, rows, &flags(&[12_022_855, 12_020_850])).is_none());
        assert!(gate(ER_BOSS_BGM, NOKRON, rows, &|_| None).is_none());
        // the Stray's flags do not open Nokron's fight and the other way round
        assert!(gate(ER_BOSS_BGM, NOKRON, rows, &flags(&[30_202_805])).is_none());
        assert!(gate(ER_BOSS_BGM, HALIGTREE_PATH, rows, &flags(&[12_022_855])).is_none());
        // another boss music id in the same map (m12_02's other boss asks 931000)
        assert!(gate(931_000, NOKRON, rows, &flags(&[12_022_855])).is_none());
        // the other bosses that ask 921100 (traced: InitializeCommonEvent 9005822 / 90005885 with 921100), each in its own map
        // with its own fog passed and even with a Mimic Tear's fog flag left on in the save: never
        let others: [(i32, u32, u32); 6] = [
            (0x0B05_0000, 11_050_850, 11_052_855),       // m11_05_00_00
            (0x0C03_0000, 12_030_800, 12_032_805),       // m12_03_00_00
            (0x3C27_3200, 1_039_500_800, 1_039_502_806), // m60_39_50_00 (90005885)
            (0x3C35_3800, 1_053_560_800, 1_053_562_806), // m60_53_56_00 (90005885)
            (0x3D2E_2600, 2_046_380_800, 2_046_382_805), // m61_46_38_00
            (0x3D33_2C00, 2_051_440_800, 2_051_442_805), // m61_51_44_00
        ];
        for (block, defeat, fog) in others {
            let on = move |f: u32| Some(f == fog || f == 12_022_855 || f == 30_202_805);
            assert!(gate(ER_BOSS_BGM, block, rows, &on).is_none(), "{block:#x} (boss {defeat})");
        }
        // no player (block 0)
        assert!(gate(ER_BOSS_BGM, 0, rows, &flags(&[12_022_855, 30_202_805])).is_none());
    }

    /// (MIMIC-MUSIC) without the party feature there is no Mimic Tear boss of ours: the gate takes nothing
    #[cfg(not(feature = "party"))]
    #[test]
    fn mm_no_party_no_gate() {
        assert!(fights().is_empty());
        assert!(gate(ER_BOSS_BGM, NOKRON, fights(), &|_| Some(true)).is_none());
        assert!(gate(ER_BOSS_BGM, HALIGTREE_PATH, fights(), &|_| Some(true)).is_none());
    }

    /// (MIMIC-MUSIC) the start and end points: Elden Ring's own SetBossBGM asks (state 0 start, 1 phase 2, -1 the defeat's stop)
    #[test]
    fn mm_starts_and_ends_where_elden_rings_boss_music_does() {
        let mut m = Machine::default();
        // another boss's ask (not gated), a gated ask with the feature off: nothing
        assert!(m.frame(&[ask(0, None)], live(100), 6000).is_empty());
        assert!(m.frame(&[ask(0, Some("mimic_tear"))], Input { on: false, ..live(150) }, 6000).is_empty());
        assert_eq!(m.phase, Phase::Off);
        // the fog passed: state 0 -> start
        assert_eq!(m.frame(&[ask(0, Some("mimic_tear"))], live(200), 6000), vec![Act::Start("mimic_tear")]);
        assert_eq!(m.phase, Phase::Playing("mimic_tear"));
        // frames of the fight; its phase 2 (60 % HP): plays on
        assert!(m.frame(&[], live(300), 6000).is_empty());
        assert_eq!(m.frame(&[ask(1, Some("mimic_tear"))], live(400), 6000), vec![Act::Phase2]);
        // the defeat: state -1 -> KH3's track fades now; Elden Ring's music given back 6 s later
        assert_eq!(m.frame(&[ask(-1, None)], live(500), 6000), vec![Act::Stop(Why::Defeat)]);
        assert!(m.frame(&[], live(6400), 6000).is_empty());
        assert_eq!(m.frame(&[], live(6500), 6000), vec![Act::Restore(Why::Defeat)]);
        assert_eq!(m.phase, Phase::Off);
        // the defeat flag alone also ends it (an ask missed)
        let mut d = Machine::default();
        d.frame(&[ask(0, Some("stray_mimic_tear"))], live(0), 6000);
        assert_eq!(d.frame(&[], Input { defeated: true, ..live(10) }, 6000), vec![Act::Stop(Why::Defeat)]);
    }

    /// (MIMIC-MUSIC) the player's death, a reload, the key switched off
    #[test]
    fn mm_ends_on_death_reload_and_off() {
        // death: KH3's track fades at once; Elden Ring's music stays silent through the game over until the load (Sora's
        // frames stop), then the event's next fog entry starts it again
        let mut m = Machine::default();
        m.frame(&[ask(0, Some("mimic_tear"))], live(0), 6000);
        assert_eq!(m.frame(&[], Input { leader_down: true, ..live(100) }, 6000), vec![Act::Stop(Why::Death)]);
        assert!(m.frame(&[], Input { leader_down: true, ..live(5000) }, 6000).is_empty());
        assert_eq!(m.frame(&[], Input { leader_down: true, sora_live: false, ..live(9000) }, 6000), vec![Act::Restore(Why::Death)]);
        assert!(m.frame(&[], Input { sora_live: false, ..live(9100) }, 6000).is_empty());
        assert_eq!(m.frame(&[ask(0, Some("mimic_tear"))], live(20_000), 6000), vec![Act::Start("mimic_tear")]);
        // the leader stands again without a load (a revive): given back
        let mut r = Machine::default();
        r.frame(&[ask(0, Some("mimic_tear"))], live(0), 6000);
        r.frame(&[], Input { leader_down: true, ..live(10) }, 6000);
        assert_eq!(r.frame(&[], live(20), 6000), vec![Act::Restore(Why::Death)]);
        // no start while the leader is down
        let mut n = Machine::default();
        assert!(n.frame(&[ask(0, Some("mimic_tear"))], Input { leader_down: true, ..live(0) }, 6000).is_empty());
        // a reload / quit / warp mid-fight: stopped and given back at once
        let mut l = Machine::default();
        l.frame(&[ask(0, Some("stray_mimic_tear"))], live(0), 6000);
        assert_eq!(l.frame(&[], Input { sora_live: false, ..live(50) }, 6000), vec![Act::Stop(Why::Left), Act::Restore(Why::Left)]);
        // the key off mid-fight
        let mut o = Machine::default();
        o.frame(&[ask(0, Some("mimic_tear"))], live(0), 6000);
        assert_eq!(o.frame(&[], Input { on: false, ..live(50) }, 6000), vec![Act::Stop(Why::Off), Act::Restore(Why::Off)]);
        // a stop ask with nothing playing: nothing
        assert!(Machine::default().frame(&[ask(-1, None), ask(-2, None)], live(0), 6000).is_empty());
    }

    /// (MIMIC-MUSIC) the loop log's passes: the intro, then each loop of [558816, 13405092)
    #[test]
    fn mm_loop_passes() {
        let end = 13_405_092.0 / 48_000.0;
        let period = (13_405_092.0 - 558_816.0) / 48_000.0;
        assert_eq!(loop_passes(0.0), 0);
        assert_eq!(loop_passes(end + 0.9), 0);
        assert_eq!(loop_passes(end + 1.1), 1);
        assert_eq!(loop_passes(end + period + 1.1), 2);
    }

    fn staged() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_tmp/weapons/MIMIC-BOSS/stage/mimic/audio")
    }

    /// (MIMIC-MUSIC) the asset: KH3's replica Xehanorts theme, packed by tools/mimic_music_pack.js into the extra root, looped at
    /// KH3's loop points, merged into a pack as fx.rs does and resolved with its material on disk
    #[test]
    fn mm_the_replicas_resolve_in_the_staged_root() {
        let stage = staged();
        let Ok(text) = std::fs::read_to_string(stage.join("manifest.json")) else {
            println!("no staged Mimic Tear music root: skipped");
            return;
        };
        assert!(text.contains(REPLICAS_ASSET) && text.contains("066_M[Quad]"));
        let extra = kh_audio::Pack::load(&stage).unwrap();
        let asset = extra.asset(REPLICAS_ASSET).expect("the Replicas asset");
        assert!(asset.looping);
        assert!((asset.volume.unwrap_or(0.0) - 0.46).abs() < 1e-6, "SEAD music Volume 0.46");
        let material = &extra.bank(asset.bank).materials[&0];
        assert_eq!(material.loop_points, Some(REPLICAS_LOOP));
        assert_eq!((material.samples, material.sample_rate, material.channels), (13_405_092, REPLICAS_RATE, 2));
        assert_eq!(extra.sound(asset.bank, asset.index).unwrap().category, 1, "KH3's BGM category");
        let mut main = kh_audio::Pack::from_json(&kh_anim::json::Json::parse(r#"{"format":"kh3-sora-audio-pack/1","banks":{},"sound_assets":{},"triggers":[]}"#).unwrap(), None).unwrap();
        assert_eq!(main.merge_party_root(&stage), Ok(true));
        let merged = main.asset(REPLICAS_ASSET).unwrap();
        let path = main.material_path(merged.bank, 0).unwrap();
        assert!(path.exists(), "{}", path.display());
        // the WAV's own smpl loop agrees
        let wav = kh_audio::wav::read(&path).unwrap();
        assert_eq!(wav.loop_points, Some(REPLICAS_LOOP));
        assert_eq!(wav.frames(), 13_405_092);
    }

    /// (MIMIC-MUSIC) THE ENGINE ITSELF (the real pack + the staged root): the music's level is Master x Music (its BGM category's
    /// config layer), Sound Effects never reaches it, it is started looped and is still playing after 10 minutes
    #[test]
    fn mm_the_music_level_and_its_loop_in_the_engine() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let (Ok(mut pack), true) = (kh_audio::Pack::load(&root.join("build/sora_pack/audio")), staged().join("manifest.json").exists()) else {
            println!("no pack / no staged Mimic Tear root: skipped");
            return;
        };
        assert_eq!(pack.merge_party_root(&staged()), Ok(true));
        let pack = std::sync::Arc::new(pack);
        for (rows, want) in [
            (crate::eroptions::SoundRows { master: 10, music: 7, sfx: 0, voice: 7 }, 0.46),
            (crate::eroptions::SoundRows { master: 10, music: 0, sfx: 7, voice: 7 }, 0.0),
        ] {
            let mut engine = kh_audio::Engine::new(pack.clone(), 1, kh_audio::Language::En);
            crate::fx::set_category_levels(&mut engine.driver, rows);
            let bgm = engine.play_asset(REPLICAS_ASSET, "test", kh_audio::AssetPlay::at(None, None)).expect("Replicas starts");
            let mut started = None;
            for _ in 0..3 {
                for command in engine.update(1.0 / 60.0, &kh_audio::Listener::default(), &|_| None) {
                    if let kh_audio::VoiceCommand::Start { sound, params, looped, .. } = command {
                        if sound == bgm {
                            started = Some((params.volume, looped));
                        }
                    }
                }
            }
            let (volume, looped) = started.expect("its voice started");
            assert!((volume - want).abs() < 1e-3, "Music {}: volume {volume}", rows.music);
            assert!(looped, "started looped (the material's loop points)");
            if want > 0.0 {
                // 10 minutes (over two passes of the loop): still playing
                for _ in 0..600 {
                    engine.update(1.0, &kh_audio::Listener::default(), &|_| None);
                }
                assert!(engine.driver.is_playing(bgm), "looped: still playing after 600 s");
                // the stop fades it: gone after the fade
                engine.driver.stop(bgm, Some(KH3_BGM_FADE));
                for _ in 0..4 {
                    engine.update(1.0, &kh_audio::Listener::default(), &|_| None);
                }
                assert!(!engine.driver.is_playing(bgm), "stopped after KH3's 2 s fade");
            }
        }
    }

    /// (MIMIC-MUSIC, PORT) both places resolve in 2.7.1.0 and 2.7.2.0, by recorded RVA and by pattern alone; the sound engine
    /// static they work on (gosound's) too
    #[cfg(windows)]
    #[test]
    fn mm_the_places_resolve_in_both_executables() {
        use crate::gamever::ExeVersion;
        use crate::havok::resolve_in;
        use pelite::pe64::PeFile;
        let mut checked = 0;
        for (rel, version) in [("build/sora_tmp/er/boss/eldenring.exe", ExeVersion::V2710), ("build/sora_tmp/er/port272/eldenring_2720.exe", ExeVersion::V2720)] {
            let Ok(bytes) = std::fs::read(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(rel)) else {
                println!("no copy of eldenring.exe {}: skipped", version.name());
                continue;
            };
            let pe = PeFile::from_bytes(&bytes).unwrap();
            for item in LOCATED.iter().chain([&crate::gosound::LOCATED[2]]) {
                let by_table = resolve_in(pe, item, Some(version));
                let alone = resolve_in(pe, item, None);
                assert_eq!(by_table, Ok(item.recorded(Some(version)).unwrap()), "{} {}", item.name, version.name());
                assert_eq!(alone, by_table, "{} by pattern alone", item.name);
                println!("{} {:<60} {:#09x}", version.name(), item.name, by_table.unwrap_or(0));
            }
            checked += 1;
        }
        println!("{checked} executables checked");
    }

    /// (MIMIC-BOSS hotfix 2, the user: "I wasn't hearing the music we chose playing") at Music 8 the voice's in-game level is
    /// above 0, and KH3's sound states that would leave its category muted for good in this port (an all-pause start with no end,
    /// the menu duck, the slow duck) are given back by the music's guard while it plays; the fight's own start at the fog gate is
    /// an ask of state 0 for that fight.
    #[cfg(feature = "party")]
    #[test]
    fn mm_the_music_is_audible_at_music_8_through_every_sound_state() {
        assert_eq!(our_start_ask(Some("mimic_tear")), Some(Ask { id: ER_BOSS_BGM, state: 0, fight: Some("mimic_tear") }));
        assert_eq!(our_start_ask(Some("nobody")), None);
        assert_eq!(our_start_ask(None), None);
        let mut m = Machine::default();
        let acts = m.frame(&[our_start_ask(Some("stray_mimic_tear")).unwrap()], Input { on: true, sora_live: true, now: 1, ..Input::default() }, 6000);
        assert_eq!(acts, vec![Act::Start("stray_mimic_tear")], "the fog gate starts it");
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let (Ok(mut pack), true) = (kh_audio::Pack::load(&root.join("build/sora_pack/audio")), staged().join("manifest.json").exists()) else {
            println!("no pack / no staged Mimic Tear root: skipped");
            return;
        };
        assert_eq!(pack.merge_party_root(&staged()), Ok(true));
        let pack = std::sync::Arc::new(pack);
        let mut engine = kh_audio::Engine::new(pack.clone(), 1, kh_audio::Language::En);
        crate::fx::set_category_levels(&mut engine.driver, crate::eroptions::SoundRows { master: 10, music: 8, sfx: 8, voice: 8 });
        let bgm = engine.play_asset(REPLICAS_ASSET, "test", kh_audio::AssetPlay::at(None, None)).expect("Replicas starts");
        let cat = engine.driver.active().into_iter().find(|a| a.key == bgm).map(|a| a.category).expect("active");
        let level = |e: &mut kh_audio::Engine| {
            for _ in 0..30 {
                e.update(1.0 / 60.0, &kh_audio::Listener::default(), &|_| None);
            }
            let a = e.driver.active().into_iter().find(|a| a.key == bgm).expect("still active");
            a.params.volume * e.driver.silence()
        };
        let base = level(&mut engine);
        assert!(base > 0.1, "Music 8: the voice sounds at {base}");
        // the sound states the core may send in a fight, one left unanswered: all-pause (the BGM category paused)
        for event in ["STATE_MENU_OPEN", "STATE_SLOW_START", "STATE_ALLPAUSE_START"] {
            assert!(engine.driver.event(event), "{event} is the pack's");
        }
        for _ in 0..30 {
            engine.update(1.0 / 60.0, &kh_audio::Listener::default(), &|_| None);
        }
        assert!(engine.driver.category_state(cat).0 <= 0.01, "muted by the unanswered all-pause: what the guard is for");
        assert!(crate::fx::music_guard(&mut engine.driver, cat), "the guard gives it back");
        let after = level(&mut engine);
        assert!(after > 0.1 && engine.driver.category_state(cat).0 > 0.9, "audible again: {after}");
        assert!(!crate::fx::music_guard(&mut engine.driver, cat), "nothing to do when it sounds");
    }
}
