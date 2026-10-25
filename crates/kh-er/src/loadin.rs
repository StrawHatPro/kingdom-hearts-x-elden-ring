//! (LDI) EVERYTHING IN PLACE ON THE FIRST VISIBLE FRAME AFTER A LOAD. The user, 2026-10-22: "Is it also possible to make the
//! characters and things like save points load instantly when the game loads? I've [seen] things like this happen when
//! fast travelling, and Donald and Goofy spawn in when Sora is finally placed properly" (picture:
//! build/sora_tmp/party/ref_fasttravel_popin.jpg: a fast-travel arrival, Sora standing, the save point still in its swirl,
//! Donald and Goofy not there yet). State note: build/sora_tmp/party/IMPL_LDI_STATE.md.
//!
//! WHAT A LOAD LOOKS LIKE FROM HERE (fast travel, Continue from the title, a respawn, KH3's game-over Continue, the save
//! point's Load, a map change with a loading screen): the player object goes, a new one comes back WHILE THE LOADING
//! SCREEN IS STILL UP (a new Sora host: `fresh`), and the game may still put it somewhere else in the first frames
//! (`relocated`). MEASURED (hud.rs IMPL_ER11): the in-game step's update is NOT CALLED during that time (about 4 s), and the
//! game fades its picture in from black about a second after the step is back. That is the black this module waits in
//! (`pause::available()` false while it is hooked).
//!
//! THE GATE (`LoadIn`, pure, unit-tested below):
//! - WAITING (a new host or a warp of the game's): while the step is not called (the loading screen) the party is HELD
//!   (`hold`): not stepped, not drawn, its bodies parked at the leader and unhittable (the park of party/glue.rs) - so no
//!   friend falls through a world whose collision is still streaming in, nobody is drawn at a stale place;
//! - as soon as the screen is about to fade in (the step is back, or the step is not hooked at all) and the leader stands
//!   (not in the wait for the ground), the hold ends in the same frame: the friends are put at their places beside the
//!   leader's FINAL feet and facing (the RESEAT, floor found under each spot), and they are drawn from that frame on: the
//!   first visible frame has them there;
//! - the LOAD-IN WINDOW (`window`: waiting, then `SETTLE` s after the placement, which covers the game's fade-in and its
//!   graces' flames starting): what our mod makes or shows in it appears SETTLED: the save points' idle effects warmed to
//!   their steady state, no entry burst for a save point he was put in, the grace he arrives at stands at once
//!   (savepoint.rs);
//! - never longer than `MAX_WAIT` s with the step running (no ground under him: he is let go, the friends placed), nor
//!   `MAX_BLACK` s under a black that never lifts (the step's hook lost): never held for good.
//!
//! KEYS (`kh-er.ini`, (INI) once `kh-er.ctl`; none needs writing, the defaults are the shipped behaviour):
//! | key | default | meaning |
//! |---|---|---|
//! | `loadin` | 1 | 0 = everything as before LDI (no hold, no load-in window) |
//! | `loadin.hold` | 1 | 0 = the friends are not held under the loading screen (stepped and placed at the host's first step, as before) |
//! | `loadin.warm` | 3.0 | seconds the save points' idle effects are warmed by when made in the window or made again where they stood (0 = off) |
//! | `loadin.arrive` | 4.0 | m: a DISCOVERED grace this near the leader in the window stands at once, before the game's flame is seen (0 = off) |
//! | `loadin.settle` | 2.0 | s of the window after the placement |
//! | `loadin.log` | 1 | the `loadin:` lines |

/// What the host knows once a frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct In {
    /// The game put its player somewhere else this frame (sora.rs `relocated`; an origin shift is not).
    pub relocated: bool,
    /// The game's in-game step is being called (`Some(false)` = the loading screen; `None` = the step is not hooked: unknown).
    pub world_step: Option<bool>,
    /// The leader stands (not in sora.rs's wait for the ground).
    pub settled: bool,
    pub dt: f32,
}

/// What the gate says for this frame.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Out {
    /// The party is held (parked, not stepped, not drawn): the screen is the loading screen.
    pub hold: bool,
    /// The placement frame: the friends are put at their final places now (the hold, if any, ends in this frame).
    pub place: bool,
    /// The load-in window: what appears now appears settled.
    pub window: bool,
    /// A line for the log (an edge of the gate).
    pub line: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Phase {
    /// A new host or a warp: waiting for the picture (seconds waited, seconds of it under the loading screen).
    Waiting { waited: f32, black: f32, told: bool },
    /// Placed: the rest of the window.
    Settling { left: f32 },
    Play,
}

/// THE GATE (one per Sora host; a new host starts WAITING).
#[derive(Clone, Debug, PartialEq)]
pub struct LoadIn {
    phase: Phase,
    /// `loadin.settle`.
    pub settle: f32,
    /// Placements made by this host (the log, the tests).
    pub placements: u32,
}

impl Default for LoadIn {
    fn default() -> LoadIn {
        LoadIn::new()
    }
}

impl LoadIn {
    /// The window after the placement (s): the game's fade-in (about 1 s) and its graces' flames starting.
    pub const SETTLE: f32 = 2.0;
    /// With the step running and no ground under the leader, the friends are placed after this long all the same (s).
    pub const MAX_WAIT: f32 = 30.0;
    /// The loading screen as this gate sees it (the step not called) never lasts longer than this (s): past it the step
    /// is taken as gone (its hook lost) and the friends are placed all the same, never held for good.
    pub const MAX_BLACK: f32 = 90.0;

    /// A new Sora host: a load (or a new game) is coming in.
    pub fn new() -> LoadIn {
        LoadIn { phase: Phase::Waiting { waited: 0.0, black: 0.0, told: false }, settle: Self::SETTLE, placements: 0 }
    }

    /// The gate is in the load-in window.
    pub fn window(&self) -> bool {
        self.phase != Phase::Play
    }

    /// The gate waits for the picture (the placement has not been made yet).
    pub fn waiting(&self) -> bool {
        matches!(self.phase, Phase::Waiting { .. })
    }

    /// ONE FRAME (sora.rs pre_physics, after the follow phase, before the party's frame).
    pub fn frame(&mut self, i: In) -> Out {
        let dt = if i.dt.is_finite() { i.dt.clamp(0.0, 0.5) } else { 0.0 };
        let mut line = None;
        if i.relocated && !self.waiting() {
            self.phase = Phase::Waiting { waited: 0.0, black: 0.0, told: false };
            line = Some("loadin: the game put its player somewhere else: the friends are placed beside his new place before they are drawn there".to_string());
        }
        match self.phase {
            Phase::Waiting { waited, black, told } => {
                let black_now = i.world_step == Some(false);
                let waited = waited + dt;
                let black = black + if black_now { dt } else { 0.0 };
                let dark = black_now && black < Self::MAX_BLACK;
                if !dark && (i.settled || waited >= Self::MAX_WAIT) {
                    self.phase = Phase::Settling { left: self.settle.max(0.0) };
                    self.placements += 1;
                    let how = if black_now {
                        format!("THE GAME'S STEP DID NOT COME BACK in {black:.0} s (its hook lost?): placed under it all the same")
                    } else if black > 0.0 { format!("the loading screen lifts after {black:.2} s of it") } else { "no loading screen".to_string() };
                    let why = if i.settled { "he stands" } else { "NO GROUND under him after the longest wait: placed all the same" };
                    return Out { hold: false, place: true, window: true, line: Some(format!("loadin: PLACED ({how}; {why}; {waited:.2} s after the load began): the friends beside the leader's final place, drawn from this frame on; the load-in window runs {:.1} s more", self.settle)) };
                }
                self.phase = Phase::Waiting { waited, black, told: told || dark };
                if dark && !told && line.is_none() {
                    line = Some("loadin: THE LOADING SCREEN is up: the friends are held (parked, not stepped, not drawn) until it lifts".to_string());
                }
                Out { hold: dark, place: false, window: true, line }
            }
            Phase::Settling { left } => {
                let left = left - dt;
                self.phase = if left <= 0.0 { Phase::Play } else { Phase::Settling { left } };
                if self.phase == Phase::Play {
                    line = Some("loadin: the load-in window is over".to_string());
                }
                Out { hold: false, place: false, window: true, line }
            }
            Phase::Play => Out { hold: false, place: false, window: false, line },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 1.0 / 60.0;

    fn f(world_step: Option<bool>, settled: bool) -> In {
        In { relocated: false, world_step, settled, dt: DT }
    }

    /// FAST TRAVEL (the user's picture): the new host comes back under the loading screen, the game moves its player once
    /// more in the dark, then the step returns: held all through the black, placed in the very frame the step is back
    /// (the picture is still black then: the game fades in after it), the window runs on for the fade-in.
    #[test]
    fn fast_travel_holds_under_the_loading_screen_and_places_when_it_lifts() {
        let mut g = LoadIn::new();
        assert!(g.window() && g.waiting());
        let mut held = 0;
        for k in 0..240 {
            let o = g.frame(In { relocated: k == 3, ..f(Some(false), k > 10) });
            assert!(o.hold && !o.place && o.window, "frame {k}: {o:?}");
            held += 1;
        }
        assert_eq!(held, 240);
        let o = g.frame(f(Some(true), true));
        assert!(!o.hold && o.place && o.window, "{o:?}");
        assert!(o.line.unwrap().contains("the loading screen lifts after 4.00 s"));
        // the window: no hold, no second placement, then play
        let frames = (0..600).take_while(|_| g.frame(f(Some(true), true)).window).count();
        assert!((118..=121).contains(&frames), "{frames}");
        assert!(!g.window() && g.placements == 1);
        assert_eq!(g.frame(f(Some(true), true)), Out::default());
    }

    /// A load whose player stands at once and no loading screen in between (a respawn the game makes without one, the
    /// step not hooked): placed in the first frame, never held.
    #[test]
    fn no_loading_screen_places_at_once_and_never_holds() {
        for step in [Some(true), None] {
            let mut g = LoadIn::new();
            let o = g.frame(f(step, true));
            assert!(o.place && !o.hold && o.window, "{step:?}: {o:?}");
            assert!(o.line.unwrap().contains("no loading screen"));
        }
    }

    /// The picture is back but the leader waits for the ground (sora.rs NoGround): no hold (the screen is not black; the
    /// party's own "wait for the ground" park hides them), placed when he stands; at most MAX_WAIT.
    #[test]
    fn the_wait_for_the_ground_is_not_a_hold_and_ends_at_the_latest_after_max_wait() {
        let mut g = LoadIn::new();
        for _ in 0..30 {
            let o = g.frame(f(Some(true), false));
            assert!(!o.hold && !o.place && o.window);
        }
        assert!(g.frame(f(Some(true), true)).place);
        let mut g = LoadIn::new();
        let n = (0..5000).take_while(|_| !g.frame(f(Some(true), false)).place).count();
        assert!((1795..=1801).contains(&n), "{n}");
        // ...but never while the screen is still the loading screen
        let mut g = LoadIn::new();
        assert!((0..3000).all(|_| { let o = g.frame(f(Some(false), false)); o.hold && !o.place }));
    }

    /// A WARP OF THE GAME'S IN PLAY (a waygate, a teleport trap; the step running): the window opens again and the friends
    /// are placed in the same frame (nothing held: the screen is not black); a warp under a loading screen holds again.
    #[test]
    fn a_warp_in_play_reopens_the_window_and_places_at_once() {
        let mut g = LoadIn::new();
        g.frame(f(Some(true), true));
        for _ in 0..200 {
            g.frame(f(Some(true), true));
        }
        assert!(!g.window());
        let o = g.frame(In { relocated: true, ..f(Some(true), true) });
        assert!(o.place && !o.hold && o.window && g.placements == 2, "{o:?}");
        let o = g.frame(In { relocated: true, ..f(Some(false), true) });
        assert!(o.hold && !o.place && g.waiting());
        assert!(g.frame(f(Some(true), true)).place);
        assert_eq!(g.placements, 3);
    }

    /// The step never comes back (its hook lost, the black never "lifts" for us): the hold ends after MAX_BLACK s all the
    /// same, the friends placed (never held for good).
    #[test]
    fn a_black_that_never_lifts_is_held_at_most_max_black() {
        let mut g = LoadIn::new();
        let n = (0..10_000).take_while(|_| g.frame(f(Some(false), true)).hold).count();
        assert!((5395..=5401).contains(&n), "{n}");
        assert!(!g.waiting() && g.placements == 1);
    }

    /// A long, stuttering load (frames of 0.5 s and more, a NaN delta): nothing panics, the hold lasts as long as the black.
    #[test]
    fn a_long_stuttering_load_is_held_until_it_lifts() {
        let mut g = LoadIn::new();
        for k in 0..400 {
            let dt = [0.0, 2.5, f32::NAN, 0.016, 1.0e9][k % 5];
            let o = g.frame(In { dt, ..f(Some(false), true) });
            assert!(o.hold && !o.place);
        }
        let o = g.frame(f(Some(true), true));
        assert!(o.place && !o.hold);
    }
}
