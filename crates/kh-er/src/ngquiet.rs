//! (CC-MUTE) THE NEW GAME'S CREATION IS SILENT AND TAKES NO INPUT. THE USER (2026-11-07): "mute all of the sounds during Elden
//! Ring's character creation screen, until our own KH pop ups appear with the new game questions", and "make sure no input is
//! taken during Elden Ring's character creation, so that user input can't mess anything up". State note:
//! build/sora_tmp/cc_mute/STATE.md.
//!
//! THE WINDOW (the game's own flow and ours, never a timer):
//! - it OPENS in the frame Elden Ring's creation begins: "Select Character Base" is made (title.rs, the
//!   `ChrMakeBaseSelectDialog` factory's hook; the same call puts newgame.rs's black cover up);
//! - it CLOSES the frame our first KH new-game pop-up is up (newgame.rs `Flow::PopUp`: `hud::title_choice_frame` drew it),
//!   and on every other way out: a back-out to the title (the title menu is made anew), a load (TitleStep PlayGame /
//!   GameStepWait, or a character exists), the new game going on with no pop-up (the choice menu unavailable), the keys
//!   switched off, the payload's stop (`shutdown`, also on a panic).
//!
//! WHAT IS SILENT while it holds:
//! - ELDEN RING'S WHOLE MIX (its creation music `m000000010`, asked in the very frame the base screen is made, SEEN in
//!   TITLE-AUDIO's runs; its menu sounds f000000000 / f000001000 / ...; the Tarnished's voice previews; any ambience): the game's
//!   own master-volume factor the title's silence already uses (gosound.rs `title_factor`, CSSound + 0x34C; titlemusic.rs owns
//!   it): the title's silence is simply KEPT through the creation instead of being given back at "a new game starts", or put
//!   on when the title music is switched off. No new address.
//! - OUR KH SOUNDS: the audio host's silence (fx.rs `sync_silence`, kh-audio's exempt marks): the title music's tail (it ends
//!   at "a new game starts", titlemusic.rs) and anything else of ours fade out over `newgame.mute_fade`; menu sounds are exempt,
//!   so the pop-up's own first sound is never cut, and the silence is lifted over `newgame.unmute_fade` when it is up.
//!
//! ELDEN RING'S MIX THROUGH THE QUESTIONS: the pop-ups are asked while the game's new-game step is HELD on black (title.rs),
//! with its creation music still running underneath, so by default Elden Ring stays silent under our pop-ups too and is
//! given back (titlemusic.rs's 1 s fade) when the game goes on to load (TitleStep PlayGame / GameStepWait), on a back-out or
//! at the payload's stop. `newgame.mute.questions = 0`: Elden Ring's mix comes back the moment the first pop-up is up.
//!
//! INPUT while the creation is pressed FOR the player (newgame.rs Base / Create / Leaving, `newgame.auto`) and while the pop-up
//! is being made ready (its UI pack still being read): the game sees nothing of the pad (`inputs::set_mask`, the XInput hook;
//! our own presses go in after the mask) nor of the keyboard and mouse (dinput_mask.rs level 3: every key, button, the wheel
//! and the mouse's movement; our own key presses pass by `dinput_mask::allow_press`). A creation the player makes himself
//! (`newgame.auto = 0`, or our creation gave up and handed the screens to him) keeps his input. The pop-up reads the pad and
//! keyboard itself (around both masks) and the masks are gone the frame it is up. Keys: `newgame.mute = 0`,
//! `newgame.block_input = 0`.

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::Mutex;

use crate::control::Control;
use crate::erlog;

/// What newgame.rs's flow is doing this frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Flow {
    /// Nothing of ours (the title, a creation the player makes himself with `newgame.auto = 0`, in game).
    #[default]
    Idle,
    /// Our automatic creation presses the screens for the player under the black cover (Base / Create / Leaving).
    Driving,
    /// Our creation gave up: the screens are the player's.
    HandedOver,
    /// The choices are to be asked: the pop-up is not up yet (its UI pack is being read).
    AskLoading,
    /// Our first KH new-game pop-up is up (or answered).
    PopUp,
    /// The choices are done (or skipped): the game goes on to its loading screen under the cover.
    Going,
    /// The cover is gone: the game is starting (or our flow gave up for this new game).
    Gone,
}

/// Why the window (or the questions' hold of Elden Ring's mix) ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Why {
    PopUp,
    Title,
    Load,
    NoPopUp,
    Off,
    Shutdown,
}

impl Why {
    pub fn words(self) -> &'static str {
        match self {
            Why::PopUp => "our first KH new-game pop-up is up",
            Why::Title => "back on the title (its menu was made anew)",
            Why::Load => "the game loads (TitleStep PlayGame / GameStepWait, or a character exists)",
            Why::NoPopUp => "the new game went on without our pop-ups",
            Why::Off => "newgame.mute and newgame.block_input are both 0",
            Why::Shutdown => "the payload stops",
        }
    }
}

/// What the flow says this frame (ticks in ms, 0 = never).
#[derive(Clone, Copy, Debug, Default)]
pub struct Signals {
    /// `newgame.mute` (the sound side).
    pub sound: bool,
    /// `newgame.block_input`.
    pub input: bool,
    /// `newgame.mute.questions`: Elden Ring's mix stays silent under our pop-ups.
    pub questions: bool,
    /// When "Select Character Base" was last made (title.rs).
    pub base_at: u64,
    /// When the title menu was last made (title.rs).
    pub top_at: u64,
    /// `CS::TitleStep`'s state (-1: none seen).
    pub state: i32,
    /// A character exists (the world is loaded).
    pub player: bool,
    pub flow: Flow,
    pub now: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Phase {
    #[default]
    Idle,
    /// Elden Ring's creation runs: everything silent.
    Creation,
    /// Our pop-ups are up: only Elden Ring's mix is still held (`newgame.mute.questions`).
    Questions,
}

/// What is to be held this frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Holds {
    /// Elden Ring's whole mix silent (titlemusic.rs's factor).
    pub er: bool,
    /// Our KH sounds silent (the audio host's silence; menu sounds exempt).
    pub kh: bool,
    /// The game takes no input (pad, keyboard, mouse).
    pub input: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    Open,
    /// The creation's window closed: (why, Elden Ring still held under the questions).
    Close(Why, bool),
    /// The questions' hold of Elden Ring's mix ended.
    QuestionsEnd(Why),
}

/// THE RULE (pure).
#[derive(Clone, Copy, Debug, Default)]
pub struct Window {
    pub phase: Phase,
    /// The base screen's tick that opened the window last (a new creation is a newer one).
    opened_base: u64,
    opened_at: u64,
}

const ST_PLAY_GAME: i32 = 5;

impl Window {
    /// One frame: what to hold, and what changed.
    pub fn frame(&mut self, s: Signals) -> (Holds, Option<Event>) {
        let load = s.player || s.state == ST_PLAY_GAME || s.state == crate::title::ST_GAME_STEP_WAIT;
        let back = self.phase != Phase::Idle && s.top_at > self.opened_at;
        let off = !s.sound && !s.input;
        let event = match self.phase {
            Phase::Idle if s.base_at != 0 && s.base_at > self.opened_base && !load && !off => {
                self.phase = Phase::Creation;
                self.opened_base = s.base_at;
                self.opened_at = s.base_at.min(s.now);
                Some(Event::Open)
            }
            Phase::Creation => {
                let why = if load {
                    Some(Why::Load)
                } else if back {
                    Some(Why::Title)
                } else if off {
                    Some(Why::Off)
                } else if s.flow == Flow::PopUp {
                    Some(Why::PopUp)
                } else if matches!(s.flow, Flow::Going | Flow::Gone) {
                    Some(Why::NoPopUp)
                } else {
                    None
                };
                why.map(|why| {
                    let keep = why == Why::PopUp && s.questions && s.sound;
                    self.phase = if keep { Phase::Questions } else { Phase::Idle };
                    Event::Close(why, keep)
                })
            }
            Phase::Questions => {
                let why = if load {
                    Some(Why::Load)
                } else if back {
                    Some(Why::Title)
                } else if !s.sound || !s.questions {
                    Some(Why::Off)
                } else if matches!(s.flow, Flow::Idle | Flow::Gone | Flow::Driving | Flow::HandedOver) {
                    Some(Why::NoPopUp)
                } else {
                    None
                };
                why.map(|why| {
                    self.phase = Phase::Idle;
                    Event::QuestionsEnd(why)
                })
            }
            Phase::Idle => None,
        };
        let creation = self.phase == Phase::Creation;
        let holds = Holds {
            er: s.sound && (creation || self.phase == Phase::Questions),
            kh: s.sound && creation,
            input: s.input && creation && matches!(s.flow, Flow::Driving | Flow::AskLoading),
        };
        (holds, event)
    }

    /// The payload stops: nothing is held any more.
    pub fn release(&mut self) -> Option<Event> {
        let was = self.phase;
        self.phase = Phase::Idle;
        match was {
            Phase::Idle => None,
            Phase::Creation => Some(Event::Close(Why::Shutdown, false)),
            Phase::Questions => Some(Event::QuestionsEnd(Why::Shutdown)),
        }
    }
}

/// THE KH SIDE'S SILENCE, with the game over's (gosound.rs) beside it (pure): either holds it; the fade is the one whose hold
/// changed last (`ng_last`: ours changed after the game over's).
pub fn combine(game_over: (bool, f32), ours: (bool, f32), ng_last: bool) -> (bool, f32) {
    let on = game_over.0 || ours.0;
    let fade = if ours.0 && !game_over.0 {
        ours.1
    } else if game_over.0 {
        game_over.1
    } else if ng_last {
        ours.1
    } else {
        game_over.1
    };
    (on, fade)
}

// ------------------------------------------------------------------------------------------ the game-side state

struct State {
    window: Window,
    /// dinput_mask.rs level 3 is ours (given back to 0 when the hold ends).
    swallowed: bool,
}

static STATE: Mutex<State> = Mutex::new(State { window: Window { phase: Phase::Idle, opened_base: 0, opened_at: 0 }, swallowed: false });
static ER_HOLD: AtomicBool = AtomicBool::new(false);
static KH_HOLD: AtomicBool = AtomicBool::new(false);
static INPUT_HOLD: AtomicBool = AtomicBool::new(false);
/// The KH side's fade now (f32 bits): the mute fade while it holds, the unmute fade after.
static KH_FADE: AtomicU32 = AtomicU32::new(0x3E80_0000);
/// When our KH hold last ended (ms tick; 0 never), for `combine`'s fade choice.
static KH_RELEASED_AT: AtomicU64 = AtomicU64::new(0);

fn now() -> u64 {
    // SAFETY: plain getter.
    unsafe { windows_sys::Win32::System::SystemInformation::GetTickCount64() }
}

fn fade_key(control: &Control, key: &str, default: f32) -> f32 {
    control.f32(key).filter(|v| v.is_finite() && (0.0..=10.0).contains(v)).unwrap_or(default)
}

/// Elden Ring's whole mix is to stay silent (titlemusic.rs keeps / puts its factor at 0 while this holds).
pub fn er_wanted() -> bool {
    ER_HOLD.load(Ordering::Relaxed)
}

/// The KH side's fade now (s): `newgame.mute_fade` while it holds, `newgame.unmute_fade` after.
pub fn kh_fade() -> f32 {
    f32::from_bits(KH_FADE.load(Ordering::Relaxed))
}

/// (fx.rs `sync_silence`) The audio host's silence: the game over's (gosound.rs) and ours together.
pub fn kh_wanted(game_over: (bool, f32)) -> (bool, f32) {
    let released = KH_RELEASED_AT.load(Ordering::Relaxed);
    let ng_last = released != 0 && now().saturating_sub(released) < 2000;
    combine(game_over, (KH_HOLD.load(Ordering::Relaxed), kh_fade()), ng_last)
}

/// (eroptions.rs) The game's keyboard and mouse are ours to hold now (its per-frame "nothing swallowed on the title" must not
/// undo it).
pub fn input_held() -> bool {
    INPUT_HOLD.load(Ordering::Relaxed)
}

fn publish(st: &mut State, holds: Holds, control: Option<&Control>) {
    if let Some(control) = control {
        let fade = if holds.kh { fade_key(control, "newgame.mute_fade", 0.25) } else { fade_key(control, "newgame.unmute_fade", 0.1) };
        KH_FADE.store(fade.to_bits(), Ordering::Relaxed);
    }
    ER_HOLD.store(holds.er, Ordering::Relaxed);
    if KH_HOLD.swap(holds.kh, Ordering::Relaxed) && !holds.kh {
        KH_RELEASED_AT.store(now(), Ordering::Relaxed);
    }
    INPUT_HOLD.store(holds.input, Ordering::Relaxed);
    if holds.input {
        crate::inputs::set_mask(u32::MAX);
        if !st.swallowed {
            crate::remap_er::ensure_installed();
        }
        crate::remap_er::set_swallow(3);
        st.swallowed = true;
    } else if st.swallowed {
        crate::remap_er::set_swallow(0);
        st.swallowed = false;
    }
}

fn tell(event: Event, holds: Holds) {
    match event {
        Event::Open => erlog!(
            "ngquiet: ELDEN RING'S CHARACTER CREATION BEGINS (\"Select Character Base\" made): {}; {}",
            if holds.er { "ALL SOUND IS SILENT (Elden Ring's whole mix by its master volume, our KH sounds by the audio host's silence)" } else { "sound as it is (newgame.mute = 0)" },
            if holds.input { "the game takes NO input (pad, keyboard, mouse) while our creation is pressed for the player" } else { "the player's input is the game's (he makes the character himself, or newgame.block_input = 0)" }
        ),
        Event::Close(why, keep) => erlog!(
            "ngquiet: the creation's silence and input hold END: {}; {}",
            why.words(),
            if keep { "Elden Ring's mix stays silent under our pop-ups (its creation music still runs under the held step) until the game loads (newgame.mute.questions = 0: back now)" } else { "Elden Ring's mix is given back" }
        ),
        Event::QuestionsEnd(why) => erlog!("ngquiet: Elden Ring's mix is given back after our pop-ups: {}", why.words()),
    }
}

/// (frame.rs, FRAME_BEGIN, every frame, right after newgame.rs's frame) The window, from the game's flow and ours.
pub fn frame(control: &Control) {
    let Ok(mut st) = STATE.lock() else { return };
    let s = Signals {
        sound: control.flag("newgame.mute") != Some(false),
        input: control.flag("newgame.block_input") != Some(false),
        questions: control.flag("newgame.mute.questions") != Some(false),
        base_at: crate::title::base_made_at(),
        top_at: crate::title::top_made_at(),
        state: crate::title::state(),
        player: crate::game::player_mut().is_some(),
        flow: crate::newgame::flow(),
        now: now(),
    };
    let (holds, event) = st.window.frame(s);
    if let Some(event) = event {
        tell(event, holds);
    }
    publish(&mut st, holds, Some(control));
}

/// (entry.rs, the payload's stop, also after a panic) Nothing stays held: the game's input back at once, the KH silence off
/// (titlemusic.rs's own shutdown gives Elden Ring's mix back).
pub fn shutdown() {
    let Ok(mut st) = STATE.lock() else { return };
    if let Some(event) = st.window.release() {
        tell(event, Holds::default());
    }
    publish(&mut st, Holds::default(), None);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(now: u64, flow: Flow) -> Signals {
        Signals { sound: true, input: true, questions: true, base_at: 0, top_at: 100, state: crate::title::ST_MENU_JOB_WAIT, player: false, flow, now }
    }

    /// The title, New Game: nothing held until the base screen is made; then everything until the first pop-up; Elden Ring's
    /// mix through the questions until the load; then nothing.
    #[test]
    fn cc_the_window_opens_on_the_base_screen_and_ends_at_the_first_popup() {
        let mut w = Window::default();
        // the title and its menus: never held
        for (t, flow) in [(1000, Flow::Idle), (2000, Flow::Idle)] {
            let (h, e) = w.frame(at(t, flow));
            assert_eq!((h, e), (Holds::default(), None));
        }
        // "Select Character Base" is made (newgame.rs takes Base the same frame)
        let s = Signals { base_at: 3000, ..at(3000, Flow::Driving) };
        let (h, e) = w.frame(s);
        assert_eq!(e, Some(Event::Open));
        assert_eq!(h, Holds { er: true, kh: true, input: true });
        // the creation is pressed, then the choices' pack is read: still all held
        assert_eq!(w.frame(Signals { now: 5000, ..s }).0, Holds { er: true, kh: true, input: true });
        assert_eq!(w.frame(Signals { now: 9000, flow: Flow::AskLoading, state: crate::title::ST_BEGIN_NEW_GAME, ..s }).0, Holds { er: true, kh: true, input: true });
        // THE FIRST POP-UP IS UP: our sounds and the input are given back; Elden Ring stays silent under it
        let (h, e) = w.frame(Signals { now: 9100, flow: Flow::PopUp, state: crate::title::ST_BEGIN_NEW_GAME, ..s });
        assert_eq!(e, Some(Event::Close(Why::PopUp, true)));
        assert_eq!(h, Holds { er: true, kh: false, input: false });
        // answered: the step goes on under the cover
        assert_eq!(w.frame(Signals { now: 12_000, flow: Flow::Going, state: crate::title::ST_BEGIN_NEW_GAME, ..s }).0, Holds { er: true, kh: false, input: false });
        // the game's loading: Elden Ring's mix back
        let (h, e) = w.frame(Signals { now: 12_100, flow: Flow::Going, state: crate::title::ST_GAME_STEP_WAIT, ..s });
        assert_eq!(e, Some(Event::QuestionsEnd(Why::Load)));
        assert_eq!(h, Holds::default());
        // in game, the same base tick: never again
        assert_eq!(w.frame(Signals { now: 30_000, flow: Flow::Idle, player: true, state: crate::title::ST_GAME_STEP_WAIT, ..s }), (Holds::default(), None));
        assert_eq!(w.frame(Signals { now: 31_000, flow: Flow::Idle, player: false, state: crate::title::ST_MENU_JOB_WAIT, ..s }), (Holds::default(), None));
    }

    /// `newgame.mute.questions = 0`: Elden Ring's mix comes back with the first pop-up.
    #[test]
    fn cc_questions_key_gives_elden_ring_back_at_the_popup() {
        let mut w = Window::default();
        let s = Signals { base_at: 3000, questions: false, ..at(3000, Flow::Driving) };
        assert_eq!(w.frame(s).1, Some(Event::Open));
        let (h, e) = w.frame(Signals { now: 9000, flow: Flow::PopUp, ..s });
        assert_eq!(e, Some(Event::Close(Why::PopUp, false)));
        assert_eq!(h, Holds::default());
        assert_eq!(w.phase, Phase::Idle);
    }

    /// Every way out releases everything: back to the title, a load, the flow going on without pop-ups, the keys, the stop.
    #[test]
    fn cc_every_exit_releases() {
        let open = |w: &mut Window, base: u64| {
            let s = Signals { base_at: base, top_at: 100, ..at(base, Flow::Driving) };
            assert_eq!(w.frame(s).1, Some(Event::Open));
            s
        };
        // back out to the title: the title menu is made anew
        let mut w = Window::default();
        let s = open(&mut w, 3000);
        let (h, e) = w.frame(Signals { now: 4000, top_at: 3900, flow: Flow::Idle, ..s });
        assert_eq!((h, e), (Holds::default(), Some(Event::Close(Why::Title, false))));
        // a NEW creation from that title opens it again
        let s = open(&mut w, 5000);
        // a load (PlayGame) / a character
        let (h, e) = w.frame(Signals { now: 6000, state: 5, ..s });
        assert_eq!((h, e), (Holds::default(), Some(Event::Close(Why::Load, false))));
        let s = open(&mut w, 7000);
        let (h, e) = w.frame(Signals { now: 8000, player: true, ..s });
        assert_eq!((h, e), (Holds::default(), Some(Event::Close(Why::Load, false))));
        // the choice menu unavailable: the game goes on without pop-ups
        let s = open(&mut w, 9000);
        let (h, e) = w.frame(Signals { now: 10_000, flow: Flow::Going, ..s });
        assert_eq!((h, e), (Holds::default(), Some(Event::Close(Why::NoPopUp, false))));
        // both keys off mid-way
        let s = open(&mut w, 11_000);
        let (h, e) = w.frame(Signals { now: 12_000, sound: false, input: false, ..s });
        assert_eq!((h, e), (Holds::default(), Some(Event::Close(Why::Off, false))));
        // the payload's stop: in the creation and under the questions
        let s = open(&mut w, 13_000);
        assert_eq!(w.release(), Some(Event::Close(Why::Shutdown, false)));
        assert_eq!(w.release(), None);
        let _ = open(&mut w, 15_000);
        let _ = w.frame(Signals { now: 16_000, flow: Flow::PopUp, ..s });
        assert_eq!(w.phase, Phase::Questions);
        assert_eq!(w.release(), Some(Event::QuestionsEnd(Why::Shutdown)));
        // under the questions: back to the title / our flow reset
        let s = open(&mut w, 17_000);
        let _ = w.frame(Signals { now: 18_000, flow: Flow::PopUp, ..s });
        let (h, e) = w.frame(Signals { now: 19_000, top_at: 18_500, flow: Flow::Idle, ..s });
        assert_eq!((h, e), (Holds::default(), Some(Event::QuestionsEnd(Why::Title))));
        let s = open(&mut w, 20_000);
        let _ = w.frame(Signals { now: 21_000, flow: Flow::PopUp, ..s });
        let (h, e) = w.frame(Signals { now: 22_000, flow: Flow::Gone, ..s });
        assert_eq!((h, e), (Holds::default(), Some(Event::QuestionsEnd(Why::NoPopUp))));
    }

    /// The input is held only while OUR creation presses the screens (or the pop-up is being readied): a creation the player
    /// makes himself, or one handed over to him, keeps his input; the sound stays silent there.
    #[test]
    fn cc_input_only_while_our_creation_drives() {
        let mut w = Window::default();
        // newgame.auto = 0: the player makes it himself (newgame's flow stays Idle)
        let s = Signals { base_at: 3000, ..at(3000, Flow::Idle) };
        let (h, e) = w.frame(s);
        assert_eq!(e, Some(Event::Open));
        assert_eq!(h, Holds { er: true, kh: true, input: false });
        // our creation gave up: the screens are his
        let (h, _) = w.frame(Signals { now: 4000, flow: Flow::HandedOver, ..s });
        assert_eq!(h, Holds { er: true, kh: true, input: false });
        // his creation is done: the pop-up is readied (input held for that moment), then up
        assert!(w.frame(Signals { now: 5000, flow: Flow::AskLoading, ..s }).0.input);
        assert_eq!(w.frame(Signals { now: 5100, flow: Flow::PopUp, ..s }).0, Holds { er: true, kh: false, input: false });
        // newgame.block_input = 0: sound only
        let mut w = Window::default();
        let s = Signals { base_at: 3000, input: false, ..at(3000, Flow::Driving) };
        assert_eq!(w.frame(s).0, Holds { er: true, kh: true, input: false });
        // newgame.mute = 0: input only
        let mut w = Window::default();
        let s = Signals { base_at: 3000, sound: false, ..at(3000, Flow::Driving) };
        assert_eq!(w.frame(s).0, Holds { er: false, kh: false, input: true });
        let (h, e) = w.frame(Signals { now: 4000, flow: Flow::PopUp, ..s });
        assert_eq!((h, e), (Holds::default(), Some(Event::Close(Why::PopUp, false))));
    }

    /// Never held outside the window: no base screen, a base tick from before a load, the keys off at the base screen.
    #[test]
    fn cc_never_held_outside_the_window() {
        let mut w = Window::default();
        for flow in [Flow::Idle, Flow::Driving, Flow::AskLoading, Flow::PopUp, Flow::Going, Flow::Gone, Flow::HandedOver] {
            assert_eq!(w.frame(at(1000, flow)), (Holds::default(), None), "no base screen made: {flow:?}");
        }
        // the base screen made while a load is under way (stale tick): not opened
        let mut w = Window::default();
        assert_eq!(w.frame(Signals { base_at: 500, state: crate::title::ST_GAME_STEP_WAIT, ..at(1000, Flow::Idle) }), (Holds::default(), None));
        assert_eq!(w.frame(Signals { base_at: 500, player: true, ..at(2000, Flow::Idle) }), (Holds::default(), None));
        // both keys off: never opened
        let mut w = Window::default();
        assert_eq!(w.frame(Signals { base_at: 3000, sound: false, input: false, ..at(3000, Flow::Driving) }), (Holds::default(), None));
    }

    /// The title music's rule is unchanged: the same base screen that ends it ("a new game starts") opens the creation's
    /// window in the same frame, so Elden Ring's mix goes straight from the title's silence into the creation's (never back
    /// in between), and a back-out restarts the music exactly when the window lets go.
    #[test]
    fn cc_the_title_musics_end_and_the_window_share_the_base_screen() {
        use crate::titlemusic::{Act, End, Machine, Signals as TitleSignals};
        let mut music = Machine::default();
        let mut w = Window::default();
        let menu = crate::title::ST_MENU_JOB_WAIT;
        assert_eq!(music.frame(TitleSignals { state: menu, top_at: 100, base_at: 0, now: 100 }), Some(Act::Start));
        assert_eq!(w.frame(at(500, Flow::Idle)).1, None);
        // New Game: the base screen is made
        let (holds, event) = w.frame(Signals { base_at: 3000, ..at(3000, Flow::Driving) });
        assert_eq!(music.frame(TitleSignals { state: menu, top_at: 100, base_at: 3000, now: 3000 }), Some(Act::Stop(End::NewGame)));
        assert_eq!(event, Some(Event::Open));
        assert!(holds.er && holds.kh, "Elden Ring's mix is held in the very frame the title's silence would have been given back");
        // back out of the creation: the title menu is made anew -> the window lets go and the music starts again
        let (holds, event) = w.frame(Signals { base_at: 3000, top_at: 6000, ..at(6000, Flow::Idle) });
        assert_eq!(event, Some(Event::Close(Why::Title, false)));
        assert_eq!(holds, Holds::default());
        assert_eq!(music.frame(TitleSignals { state: menu, top_at: 6000, base_at: 3000, now: 6000 }), Some(Act::Start));
    }

    /// The KH side's silence: either hold keeps it; the fade is the last changer's.
    #[test]
    fn cc_kh_silence_combines_with_the_game_over() {
        assert_eq!(combine((false, 0.5), (false, 0.1), false), (false, 0.5));
        assert_eq!(combine((false, 0.5), (true, 0.25), false), (true, 0.25));
        assert_eq!(combine((true, 1.0), (true, 0.25), false), (true, 1.0));
        assert_eq!(combine((false, 0.5), (false, 0.1), true), (false, 0.1), "ours just ended: its unmute fade");
        assert_eq!(combine((true, 1.0), (false, 0.1), true), (true, 1.0));
    }

    /// The game-side state: the shutdown leaves nothing held.
    #[test]
    fn cc_shutdown_leaves_nothing_held() {
        ER_HOLD.store(true, Ordering::Relaxed);
        KH_HOLD.store(true, Ordering::Relaxed);
        INPUT_HOLD.store(true, Ordering::Relaxed);
        STATE.lock().unwrap().window.phase = Phase::Creation;
        shutdown();
        assert!(!er_wanted() && !input_held() && !kh_wanted((false, 0.5)).0);
        assert_eq!(STATE.lock().unwrap().window.phase, Phase::Idle);
    }
}
