//! (GOS) THE GAME OVER'S SILENCE. THE USER (2026-11-03): "silence all music and other SFX when on death screen", and
//! then "(except for menu SFX, dont silence that)".
//!
//! WHAT IS SILENCED, from the frame the game over takes over (the leader's death) until the load / respawn / title takes
//! the picture away (the game's loading screen, or our HUD frames stopping), over `gameover.mute_fade` s (default 1.0:
//! KH3's own plate rises to full over 1.0 s, `0x140EAD210` step 0, so the world is silent when the screen is black):
//! - ELDEN RING'S whole mix (its music, ambience, SFX, voices) through THE GAME'S OWN MASTER-VOLUME CONTROL (below);
//! - OUR KH SOUNDS (Sora's and the party's voices, battle and world sounds, the alarm): kh-audio's host silence
//!   (`HostAudio::set_silence`), which spares every menu sound started while it holds.
//!
//! WHAT STILL PLAYS (KH3's game over as the reference: its game mode starts the loop SE `se00000_046` (static SE 21) at
//! the death and fades its BGM out (`0x1458221C0(2.0)`); the world is slowed to 0.01 and then hidden): the game-over loop
//! SE 21 and EVERY MENU SOUND of ours (the menu's open SE 2, cursor SE 0, decide SE 1, cancel, refused, the Load screen's
//! sounds): they are the HUD's commands (`fx::ui_sounds` -> `HostAudio::ui_commands`), marked by their own sound, so no
//! category or bus they share with the world can catch them. Our audio is our own output stream (kh_audio's mixer, not
//! the game's Wwise): Elden Ring's master volume never reaches it.
//!
//! THE GAME'S MASTER VOLUME (READ in eldenring.exe 2.7.1.0 and 2.7.2.0, not guessed): CSSound (the static PlaySE uses,
//! ersound.rs) keeps FIVE multiplied master factors at +0x33C .. +0x34C (its constructor 0xDA5220 sets all five to 1.0).
//! Their product, clamped to 0..1, goes to the sound engine's `SetMasterVolume` (0xDC69C0: the Wwise RTPC named
//! "MasterVolume", its own linear fade of the given seconds; the Options' Master Volume is ANOTHER RTPC,
//! "IngameMasterVolume", 0xDC67C0, fed from GameSettings + 0x20, so the player's options are never touched or saved).
//! The game's setters: +0x33C (0xDACBD0, many callers), +0x344 (0xDACDD0: the sound update's background mute, every
//! frame), +0x348 (0xDACCD0: the window-close mute, WM_CLOSE in the window procedure). +0x340 and +0x34C are written by
//! nothing but the constructor: +0x340 IS OURS. The silence writes 0 there and asks the game's own +0x348 setter to
//! send the product again (with +0x348's current value, so that factor stays as it is), with our fade; the restore puts
//! back exactly what +0x340 held and sends again. The game's later changes of its own factors multiply ours in (a
//! loading screen's fade cannot undo the silence, and the silence never undoes theirs).
//!
//! EVERY WAY OUT gives the sound back: the scene ends (the new character has the picture), the game's loading screen
//! begins (Continue, Load), our HUD frames stop for 1 s (the title, a respawn, the HUD switched off), the key
//! `gameover.silence = 0`, the payload's stop (`shutdown`). `gameover.unmute_fade` (default 0.5 s: the plate's own
//! leave). Never a second silence until the scene is over.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Mutex;
use std::time::Instant;

use crate::control::Control;
use crate::erlog;
use crate::havok::{locate, Located};

// ------------------------------------------------------------------------------------------ the state machine (pure)

/// What the machine asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Act {
    Mute,
    Restore,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Phase {
    /// No game over (or the key is off).
    #[default]
    Idle,
    /// The silence is in force.
    Silent,
    /// The silence was given back while the game over is still being carried out (a loading screen, a respawn): it does
    /// not come again until the scene is over.
    Done,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Silence {
    pub phase: Phase,
}

impl Silence {
    /// One HUD frame. `scene`: a game over is under way (from the death until the last plate has left); `picture`: it
    /// has the screen (no loading screen of the game's).
    pub fn frame(&mut self, scene: bool, picture: bool) -> Option<Act> {
        match self.phase {
            Phase::Idle if scene && picture => {
                self.phase = Phase::Silent;
                Some(Act::Mute)
            }
            Phase::Silent if !scene => {
                self.phase = Phase::Idle;
                Some(Act::Restore)
            }
            Phase::Silent if !picture => {
                self.phase = Phase::Done;
                Some(Act::Restore)
            }
            Phase::Done if !scene => {
                self.phase = Phase::Idle;
                None
            }
            _ => None,
        }
    }

    /// Our HUD frames stopped (the title, a respawn), the key went off, the payload stops: the sound is given back now.
    pub fn release(&mut self) -> Option<Act> {
        if self.phase == Phase::Silent {
            self.phase = Phase::Done;
            Some(Act::Restore)
        } else {
            None
        }
    }
}

// ------------------------------------------------------------------------------------------ Elden Ring's master volume

/// The game's own window-close mute: `mov rcx, [CSSound]; test rcx, rcx; je; xorps xmm2; xorps xmm1; call <the +0x348
/// setter>; xor eax, eax` (the window procedure's WM_CLOSE). Capture 1 = CSSound's static, 2 = the setter.
const PAT_CLOSE_MUTE: &str = "48 8b 0d $ { ' } 48 85 c9 74 0b 0f 57 d2 0f 57 c9 e8 $ { ' } 33 c0";
/// Inside that setter: the product of the five factors, then the sound engine's static (capture 1) whose `+0x324` is the
/// "MasterVolume" value last sent (read for the log only).
const PAT_ENGINE: &str = "f3 0f 11 81 48 03 00 00 c7 44 24 48 00 00 00 00 f3 0f 59 89 4c 03 00 00 f3 0f 59 89 44 03 00 00 f3 0f 59 c8 0f 2f e1 f3 0f 11 4c 24 58 76 07 48 8d 5c 24 48 eb 11 0f 2f cb 48 8d 5c 24 40 48 8d 44 24 58 48 0f 46 d8 48 8b 0d $ { ' } 48 85 c9";

pub(crate) const LOCATED: [Located; 3] = [
    Located { name: "CSSound static (the window-close mute)", pattern: PAT_CLOSE_MUTE, capture: 1, rva_2_7_1_0: 0x03d8_7d48, rva_2_7_2_0: 0x03d8_ad68 },
    Located { name: "CSSound master factor +0x348 setter (the window-close mute)", pattern: PAT_CLOSE_MUTE, capture: 2, rva_2_7_1_0: 0x00da_b5e0, rva_2_7_2_0: 0x00da_ccd0 },
    Located { name: "sound engine static (the MasterVolume RTPC)", pattern: PAT_ENGINE, capture: 1, rva_2_7_1_0: 0x03d8_8328, rva_2_7_2_0: 0x03d8_b348 },
];

/// The factor of CSSound's that is ours, and the one whose setter sends the product.
const OUR_FACTOR: usize = 0x340;
const CLOSE_FACTOR: usize = 0x348;
/// The sound engine's "MasterVolume" value last sent.
const ENGINE_MASTER: usize = 0x324;

type FactorSetter = unsafe extern "C" fn(*mut core::ffi::c_void, f32, f32);

#[derive(Clone, Copy)]
struct Places {
    sound_static: usize,
    setter: usize,
    engine_static: Option<usize>,
}

static PLACES: std::sync::OnceLock<Option<Places>> = std::sync::OnceLock::new();

fn places() -> Option<Places> {
    *PLACES.get_or_init(|| {
        let p = Places { sound_static: locate(&LOCATED[0])?, setter: locate(&LOCATED[1])?, engine_static: locate(&LOCATED[2]) };
        erlog!("gameover: (GOS) Elden Ring's master-volume factors located (CSSound + 0x33C .. 0x34C, the window-close mute's setter)");
        Some(p)
    })
}

fn read_ptr(at: usize) -> usize {
    // SAFETY: `at` is a static of the running image (located by its pattern); a plain pointer word.
    unsafe { std::ptr::read_volatile(at as *const usize) }
}

fn read_f32(at: usize) -> f32 {
    // SAFETY: a float field of a live game object whose pointer was checked by the caller.
    unsafe { std::ptr::read_volatile(at as *const f32) }
}

/// The "MasterVolume" value the sound engine last sent (`None`: not located / not made).
fn engine_master(p: &Places) -> Option<f32> {
    let engine = read_ptr(p.engine_static?);
    crate::game::plausible(engine).then(|| read_f32(engine + ENGINE_MASTER))
}

/// (for the log) CSSound's five master factors now (+0x33C .. +0x34C): the product is what the game sends.
fn factors_text() -> String {
    let Some(p) = places() else { return "not located".to_string() };
    let sound = read_ptr(p.sound_static);
    if !crate::game::plausible(sound) {
        return "CSSound not made".to_string();
    }
    let f: Vec<String> = (0..5).map(|k| format!("{:.2}", read_f32(sound + 0x33C + 4 * k))).collect();
    format!("factors +0x33C..+0x34C [{}]", f.join(", "))
}

/// Our factor written and the product sent again by the game's own setter, faded over `fade` s. `Some(before)`: done.
fn er_set(value: f32, fade: f32) -> Option<(usize, f32)> {
    let p = places()?;
    let sound = read_ptr(p.sound_static);
    if !crate::game::plausible(sound) {
        return None;
    }
    let before = read_f32(sound + OUR_FACTOR);
    let close = read_f32(sound + CLOSE_FACTOR);
    // SAFETY: game thread (the HUD frame / FRAME_BEGIN); CSSound is the game's own singleton (pointer checked); the
    // setter is the game's own (its bytes checked for this version) and takes (CSSound, its factor's value, fade
    // seconds); +0x348's own value is handed back to it unchanged.
    unsafe {
        std::ptr::write_volatile((sound + OUR_FACTOR) as *mut f32, value);
        std::mem::transmute::<usize, FactorSetter>(p.setter)(sound as *mut core::ffi::c_void, close, fade);
    }
    Some((sound, before))
}

/// (TITLE-AUDIO) THE TITLE'S FACTOR: CSSound + 0x34C, the other factor nothing but the constructor writes (2.7.1.0 and
/// 2.7.2.0), so the title's silence (titlemusic.rs) and the game over's (+0x340) never touch each other's value; written and
/// sent by the game's own +0x348 setter exactly as `er_set`. `Some((CSSound, before))`: done.
const TITLE_FACTOR: usize = 0x34C;

pub(crate) fn title_factor(value: f32, fade: f32) -> Option<(usize, f32)> {
    let p = places()?;
    let sound = read_ptr(p.sound_static);
    if !crate::game::plausible(sound) {
        return None;
    }
    let before = read_f32(sound + TITLE_FACTOR);
    let close = read_f32(sound + CLOSE_FACTOR);
    // SAFETY: as `er_set`: game thread (FRAME_BEGIN), CSSound checked, the game's own setter (bytes checked for this version),
    // +0x348's own value handed back to it unchanged.
    unsafe {
        std::ptr::write_volatile((sound + TITLE_FACTOR) as *mut f32, value);
        std::mem::transmute::<usize, FactorSetter>(p.setter)(sound as *mut core::ffi::c_void, close, fade);
    }
    Some((sound, before))
}

/// (TITLE-AUDIO) CSSound now (0: not located / not made).
pub(crate) fn sound_now() -> usize {
    places().map_or(0, |p| read_ptr(p.sound_static))
}

/// (TITLE-AUDIO) The five master factors and the MasterVolume RTPC, for a log line.
pub(crate) fn factors_now() -> String {
    format!("{}; MasterVolume RTPC {}", factors_text(), places().and_then(|p| engine_master(&p)).map_or("not read".to_string(), |v| format!("{v:.2}")))
}

// ------------------------------------------------------------------------------------------ the game-side state

struct State {
    machine: Silence,
    /// The last HUD frame that asked.
    asked_at: Option<Instant>,
    /// What our factor of Elden Ring's held before the silence (and the CSSound it was on).
    er_before: Option<(usize, f32)>,
    /// (the log) When to say once what the game's master volume is some seconds after the restore.
    check_at: Option<Instant>,
}

static STATE: Mutex<State> = Mutex::new(State { machine: Silence { phase: Phase::Idle }, asked_at: None, er_before: None, check_at: None });
/// (GOS) What our audio side is to do: the silence in force, and its fade (f32 bits).
static KH_ON: AtomicBool = AtomicBool::new(false);
static KH_FADE: AtomicU32 = AtomicU32::new(0x3F80_0000);

/// The KH side's silence: (in force, fade s) (fx.rs applies it to the audio host whenever it differs).
pub fn kh_wanted() -> (bool, f32) {
    (KH_ON.load(Ordering::Relaxed), f32::from_bits(KH_FADE.load(Ordering::Relaxed)))
}

fn fades(control: &Control) -> (f32, f32) {
    let ok = |v: f32| v.is_finite() && (0.0..=10.0).contains(&v);
    (control.f32("gameover.mute_fade").filter(|v| ok(*v)).unwrap_or(1.0), control.f32("gameover.unmute_fade").filter(|v| ok(*v)).unwrap_or(0.5))
}

fn carry_out(state: &mut State, act: Act, fade: f32, why: &str) {
    KH_FADE.store(fade.to_bits(), Ordering::Relaxed);
    match act {
        Act::Mute => {
            KH_ON.store(true, Ordering::Relaxed);
            let er = er_set(0.0, fade);
            if let Some((sound, before)) = er {
                // (a value the game had left there itself is put back exactly; 1.0 is what its constructor writes)
                state.er_before = Some((sound, before));
            }
            erlog!(
                "gameover: (GOS) THE SILENCE ({why}): Elden Ring's whole mix fades out over {fade:.2} s by its own master volume ({}); our KH sounds fade too, the game-over loop and the menus' sounds go on",
                match er {
                    Some((_, before)) => format!("CSSound + 0x340 {before:.2} -> 0, the game's +0x348 setter sent it; {}; MasterVolume RTPC {}", factors_text(), places().and_then(|p| engine_master(&p)).map_or("not read".to_string(), |v| format!("{v:.2}"))),
                    None => "NOT located / not made: Elden Ring's sound stays".to_string(),
                }
            );
        }
        Act::Restore => {
            KH_ON.store(false, Ordering::Relaxed);
            let line = match state.er_before.take() {
                Some((sound, before)) => {
                    let now = places().map_or(0, |p| read_ptr(p.sound_static));
                    if now == sound {
                        let _ = er_set(before, fade);
                        format!("CSSound + 0x340 back to {before:.2}; {}; MasterVolume RTPC {} (the product of the factors: a loading screen's own fade of the game's may still hold it down)", factors_text(), places().and_then(|p| engine_master(&p)).map_or("not read".to_string(), |v| format!("{v:.2}")))
                    } else {
                        "CSSound was made anew (its factors are 1.0 again): nothing to put back".to_string()
                    }
                }
                None => "Elden Ring's side was not silenced".to_string(),
            };
            state.check_at = Some(Instant::now() + std::time::Duration::from_secs(8));
            erlog!("gameover: (GOS) THE SOUND IS GIVEN BACK ({why}) over {fade:.2} s: {line}; our KH sounds too");
        }
    }
}

/// (hud.rs, every HUD frame) `scene`: a game over is under way; `picture`: it has the screen (no loading screen).
pub fn ask(control: &Control, scene: bool, picture: bool) {
    let Ok(mut state) = STATE.lock() else { return };
    state.asked_at = Some(Instant::now());
    let on = control.flag("gameover.silence") != Some(false);
    let (mute_fade, unmute_fade) = fades(control);
    let act = if on { state.machine.frame(scene, picture) } else { state.machine.release() };
    if let Some(act) = act {
        let why = match (act, on, scene) {
            (Act::Mute, _, _) => "the game over has the screen",
            (Act::Restore, false, _) => "gameover.silence = 0",
            (Act::Restore, _, false) => "the game over is over",
            (Act::Restore, _, true) => "the game's loading screen has the picture",
        };
        carry_out(&mut state, act, if act == Act::Mute { mute_fade } else { unmute_fade }, why);
    }
}

/// (frame.rs, FRAME_BEGIN: every frame, the title and loading screens too) The watchdog: our HUD frames stopped for 1 s
/// while the silence holds (the title, a respawn, the HUD switched off): the sound is given back.
pub fn frame(control: &Control) {
    let Ok(mut state) = STATE.lock() else { return };
    if state.check_at.is_some_and(|at| Instant::now() >= at) {
        state.check_at = None;
        erlog!("gameover: (GOS) 8 s after the sound was given back: {}; MasterVolume RTPC {}", factors_text(), places().and_then(|p| engine_master(&p)).map_or("not read".to_string(), |v| format!("{v:.2}")));
    }
    if state.machine.phase != Phase::Silent || state.asked_at.is_some_and(|at| at.elapsed().as_millis() < 1000) {
        return;
    }
    if let Some(act) = state.machine.release() {
        carry_out(&mut state, act, fades(control).1, "our HUD frames stopped: the load / respawn / title has the picture");
    }
}

/// (entry.rs, the payload's stop) The sound is given back at once.
pub fn shutdown() {
    let Ok(mut state) = STATE.lock() else { return };
    if let Some(act) = state.machine.release() {
        carry_out(&mut state, act, 0.0, "the payload stops");
    }
    state.machine = Silence::default();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_silence_begins_with_the_game_over_and_ends_on_every_way_out() {
        // the death: the scene has the screen -> mute, once
        let mut s = Silence::default();
        assert_eq!(s.frame(false, true), None);
        assert_eq!(s.frame(true, true), Some(Act::Mute));
        assert_eq!(s.frame(true, true), None);
        // Continue / Load: the game's loading screen takes the picture -> restore; the black plate after the load (the
        // scene still on until the new character has the picture) does NOT silence again
        assert_eq!(s.frame(true, false), Some(Act::Restore));
        assert_eq!(s.phase, Phase::Done);
        assert_eq!(s.frame(true, true), None);
        // the scene is over: ready for the next death
        assert_eq!(s.frame(false, true), None);
        assert_eq!(s.phase, Phase::Idle);
        assert_eq!(s.frame(true, true), Some(Act::Mute));
        // the scene ends with the picture still ours (the plate left over the world) -> restore, idle
        assert_eq!(s.frame(false, true), Some(Act::Restore));
        assert_eq!(s.phase, Phase::Idle);
        // the title / a respawn / the payload's stop: our HUD frames stop -> release, once
        assert_eq!(s.frame(true, true), Some(Act::Mute));
        assert_eq!(s.release(), Some(Act::Restore));
        assert_eq!(s.release(), None);
        assert_eq!(s.frame(true, true), None, "no second silence in the same game over");
        assert_eq!(s.frame(false, false), None);
        assert_eq!(s.phase, Phase::Idle);
        // nothing to give back when nothing was taken
        assert_eq!(Silence::default().release(), None);
    }

    #[test]
    fn the_game_state_restores_on_shutdown_and_the_watchdog() {
        // (the pure side of `ask` / `frame` / `shutdown`: no game here, Elden Ring's side is "not located")
        let control = Control::default();
        ask(&control, true, true);
        assert!(kh_wanted().0);
        assert!((kh_wanted().1 - 1.0).abs() < 1e-6);
        // the watchdog waits a second of no HUD frames
        frame(&control);
        assert!(kh_wanted().0);
        STATE.lock().unwrap().asked_at = Some(Instant::now() - std::time::Duration::from_millis(1500));
        frame(&control);
        assert!(!kh_wanted().0);
        assert!((kh_wanted().1 - 0.5).abs() < 1e-6);
        // a new game over, then the payload's stop
        ask(&control, false, true);
        ask(&control, true, true);
        assert!(kh_wanted().0);
        shutdown();
        assert!(!kh_wanted().0);
        assert_eq!(STATE.lock().unwrap().machine.phase, Phase::Idle);
    }
}
