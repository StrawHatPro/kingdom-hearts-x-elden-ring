//! (IMPL_ER38) THE MAP: every request to open Elden Ring's map and its outcome (`map:` lines), the game's window focus,
//! and a guard against a state of ours that keeps View from the game.
//!
//! WHAT THE USER'S LONG SESSION PROVED (er\logs\kh-er.log, build 3F58F44B7AC1, 11070 s): the save point's "Select an Area"
//! opened the map on its FIRST press, 0.03 s later, at 295, 1259, 9579, 10083, 10127 and 10298 s; it did NOT open at
//! 10004.8 - 10008.4 (4 presses), 10010.4 - 10014.0 (4 presses) and 10051.7 - 10054.1 (3 presses). In those windows the
//! game read the pad through our hook in every frame (61 of 61 calls a second) and the presses were made, but every
//! SECOND press went by the pad hook instead of the keyboard, which `inputs::press_for_er_try` does only when THE GAME'S
//! WINDOW IS NOT THE FOREGROUND WINDOW: Elden Ring was in the background (it takes no pad input then; it goes on reading
//! XInput), while Sora, whose pad read goes around the game, kept running and opening our menus as if nothing were wrong
//! (the window was lost around 9981 - 10003: the camp menu was up for 5 min and Windows reported the "Headphones" device
//! gone at 9980.97). Nothing of ours was stuck: no mask on View, the body idle (animation 0), the world running.
//!
//! WHAT IS DONE HERE:
//! - FOCUS (`Focus`): the game's window not in front for 0.25 s -> Sora takes no pad input either, as Elden Ring itself
//!   (and Kingdom Hearts III) do: the player sees at once that the game has lost the window instead of finding out later
//!   that the map, the prompts and the game's menus no longer answer. `input:` lines at both edges, with the window that
//!   has the foreground. `in.focus = 0`: Sora's pad works in the background as before.
//! - REQUESTS (`Requests`): a View press in the field (a real down-edge while the game's interface says gameplay) and
//!   each press the save point's map flow makes (`travel::from_save_point`) is a request; its outcome is logged: the
//!   game's interface left gameplay within 2 s (the map is up) or not, with what stood in the way (the window, our mask,
//!   the body's animation / ladder state, the world standing still, no pad read by the game).
//! - THE GUARD: View hidden from the game by a state of ours with NO menu of ours up (not the camp / save menu, not the
//!   game-over menu) while the game's interface says gameplay, for more than `STUCK_AFTER` s, and the player presses
//!   View: that state has stuck. The press is made for the game through the pad hook, and the hiding is lifted (the
//!   gameplay mask instead) until the state goes away by itself; said with `!!! map:`. `map.guard = 0`: off.

use crate::erlog;

/// Facts of one frame (read by sora.rs).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Facts {
    /// The game's window is the foreground window.
    pub foreground: bool,
    /// Our mask hides View from the game this frame.
    pub view_hidden: bool,
    /// A menu of ours (camp / save / game over) is up: View is rightly ours then.
    pub menu_ours: bool,
    /// The save point's cover masks the whole pad (IMPL_ER36).
    pub cover: bool,
    /// The game's interface state (3 = gameplay).
    pub hud_state: u8,
    /// The body's animation and ladder state.
    pub body_anim: i32,
    pub ladder: i8,
    /// The game's world stands still (a window of its, our pause).
    pub world_still: bool,
    /// The game reads a pad through our hook.
    pub er_reads_pad: bool,
}

impl Facts {
    fn gameplay(&self) -> bool {
        self.hud_state == 3
    }

    /// What stood in the way of a request, in words (empty: nothing of ours).
    fn obstacles(&self) -> Vec<String> {
        let mut out = Vec::new();
        if !self.foreground {
            out.push("THE GAME'S WINDOW IS NOT IN FRONT (Elden Ring takes no pad input in the background)".to_string());
        }
        if self.view_hidden {
            out.push(format!("View is hidden from the game by our mask ({})", if self.menu_ours { "a menu of ours is up" } else if self.cover { "the save point's story cover" } else { "no menu of ours is up" }));
        }
        if self.ladder >= 0 {
            out.push(format!("the body is on a ladder (state {} {})", self.ladder, crate::ladder::state_name(self.ladder)));
        } else if self.body_anim != 0 {
            out.push(format!("the body plays animation {} (not its idle 0)", self.body_anim));
        }
        if self.world_still {
            out.push("the world stands still".to_string());
        }
        if !self.er_reads_pad {
            out.push("the game reads no pad through our hook".to_string());
        }
        out
    }
}

/// Where a request came from.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Source {
    View,
    SavePoint,
}

#[derive(Clone, Copy, Debug)]
struct Pending {
    n: u32,
    source: Source,
    age: f32,
    presses: u32,
    at_press: Facts,
    /// Frames of the window in which the game's window was not in front.
    background: u32,
    frames: u32,
}

/// The requests and their outcomes (pure; returns the log lines).
#[derive(Debug, Default)]
pub struct Requests {
    count: u32,
    pending: Option<Pending>,
    view_before: bool,
}

impl Requests {
    /// The game's interface must leave gameplay this soon after a press.
    pub const WITHIN: f32 = 2.0;

    /// One frame. `view_raw`: View physically down (the real pad); `sp_press`: the save point's map flow pressed View
    /// for the game this frame.
    pub fn frame(&mut self, view_raw: bool, sp_press: bool, facts: Facts, dt: f32) -> Vec<String> {
        let mut lines = Vec::new();
        let edge = view_raw && !self.view_before;
        self.view_before = view_raw;
        // (a View press while the game's map / a menu of its is up closes it: no request)
        let field_press = edge && facts.gameplay() && !facts.menu_ours; // (View in a menu of ours is that menu's)
        if field_press || sp_press {
            let source = if sp_press { Source::SavePoint } else { Source::View };
            match self.pending.as_mut() {
                Some(p) if p.source == source || sp_press => {
                    // another press of the same request (the save point's retries; a second View inside the window)
                    p.presses += 1;
                    p.age = 0.0;
                    p.source = source;
                }
                _ => {
                    self.count += 1;
                    self.pending = Some(Pending { n: self.count, source, age: 0.0, presses: 1, at_press: facts, background: 0, frames: 0 });
                    let obstacles = facts.obstacles();
                    lines.push(format!(
                        "map: request #{} ({}): the game's interface hud_state {}, its window {}{}",
                        self.count,
                        if sp_press { "the save point's Select an Area: View pressed for the game" } else { "View in the field" },
                        facts.hud_state,
                        if facts.foreground { "in front" } else { "NOT IN FRONT" },
                        if obstacles.is_empty() { String::new() } else { format!(" | in the way: {}", obstacles.join("; ")) },
                    ));
                }
            }
            return lines;
        }
        let Some(p) = self.pending.as_mut() else { return lines };
        p.age += dt;
        p.frames += 1;
        p.background += u32::from(!facts.foreground);
        if !facts.gameplay() && facts.hud_state != 255 {
            lines.push(format!("map: request #{}: THE MAP IS UP ({} press(es); the game's interface hud_state {} after {:.2} s)", p.n, p.presses, facts.hud_state, p.age));
            self.pending = None;
        } else if p.age > Self::WITHIN {
            let mut why = p.at_press.obstacles();
            if p.background > 0 && p.at_press.foreground {
                why.push(format!("the game's window was not in front in {} of {} frame(s) after the press", p.background, p.frames));
            }
            lines.push(format!(
                "!!! map: request #{}: THE MAP DID NOT OPEN ({} press(es), {:.1} s after the last): {}",
                p.n,
                p.presses,
                p.age,
                if why.is_empty() { "nothing of ours stood in the way (the game refused its map in this state: its body's animation 0, the world running, the pad read)".to_string() } else { why.join("; ") },
            ));
            self.pending = None;
        }
        lines
    }
}

/// The game's window focus and Sora's pad (pure).
#[derive(Debug, Default)]
pub struct Focus {
    /// Seconds the window has been out of front (0 while in front).
    out_for: f32,
    gated: bool,
}

impl Focus {
    /// Out of front this long -> Sora takes no pad input (a short flicker of the foreground does not count).
    pub const AFTER: f32 = 0.25;

    /// One frame: returns (Sora takes the pad, the log line of an edge).
    pub fn frame(&mut self, foreground: bool, rule_on: bool, dt: f32) -> (bool, Option<bool>) {
        self.out_for = if foreground { 0.0 } else { self.out_for + dt };
        let gated = rule_on && self.out_for >= Self::AFTER;
        let edge = (gated != self.gated).then_some(gated);
        self.gated = gated;
        (!gated, edge)
    }
}

/// View kept from the game by a state of ours without a menu (pure).
#[derive(Debug, Default)]
pub struct Guard {
    hidden_for: f32,
    tripped: bool,
}

impl Guard {
    /// That long without a menu of ours = stuck.
    pub const STUCK_AFTER: f32 = 10.0;

    /// One frame. Returns: (press View for the game now, lift our hiding this frame, the log line).
    pub fn frame(&mut self, view_edge: bool, facts: Facts, on: bool, dt: f32) -> (bool, bool, Option<String>) {
        let suspicious = facts.view_hidden && !facts.menu_ours && facts.gameplay();
        if !suspicious {
            let line = std::mem::take(&mut self.tripped).then(|| format!("map: the state that kept View from the game is gone after {:.0} s: our mask is as it should be again", self.hidden_for));
            self.hidden_for = 0.0;
            return (false, false, line);
        }
        self.hidden_for += dt;
        if self.tripped {
            return (false, true, None);
        }
        if on && view_edge && self.hidden_for > Self::STUCK_AFTER {
            self.tripped = true;
            let who = if facts.cover { "the save point's story cover" } else { "a state of ours" };
            return (true, true, Some(format!("!!! map: View has been hidden from the game for {:.0} s by {who} with NO menu of ours up and the game's interface at gameplay: THAT STATE IS STUCK. View is pressed for the game now and our hiding is lifted (the gameplay mask) until that state ends (map.guard = 0: off)", self.hidden_for)));
        }
        (false, false, None)
    }
}

/// All of it, as sora.rs holds it.
#[derive(Debug, Default)]
pub struct MapWatch {
    pub requests: Requests,
    pub focus: Focus,
    pub guard: Guard,
    sp_presses: u32,
}

impl MapWatch {
    /// The pad gate (once a frame, before the pad is used). Returns: Sora takes the real pad this frame.
    pub fn pad_gate(&mut self, control: &crate::control::Control, dt: f32) -> bool {
        let foreground = crate::inputs::game_in_foreground();
        let (takes, edge) = self.focus.frame(foreground, control.flag("in.focus") != Some(false), dt);
        if let Some(gated) = edge {
            if gated {
                erlog!("input: THE GAME'S WINDOW IS NOT IN FRONT (the foreground window: {}): Elden Ring takes no pad input now, and neither does Sora (in.focus = 0: he does); click or Alt+Tab into the game", crate::inputs::foreground_text());
            } else {
                erlog!("input: the game's window is in front again: Sora and Elden Ring take the pad");
            }
        }
        takes
    }

    /// Once a frame after the masks are set. `view_raw`: View down on the real pad; `gameplay_mask`: the mask sora.rs
    /// computed for gameplay (before a cover); `facts` without the mask (read here).
    pub fn frame(&mut self, control: &crate::control::Control, view_raw: bool, gameplay_mask: u32, mut facts: Facts, dt: f32) {
        facts.view_hidden = crate::inputs::current_mask() & u32::from(crate::inputs::XI_BACK) != 0;
        facts.foreground = crate::inputs::game_in_foreground();
        facts.er_reads_pad = crate::inputs::er_reads_pad();
        let sp = crate::travel::save_point_presses();
        let sp_press = sp != self.sp_presses;
        self.sp_presses = sp;
        let edge = view_raw && !self.requests.view_before;
        let (press, lift, line) = self.guard.frame(edge, facts, control.flag("map.guard") != Some(false), dt);
        if let Some(line) = line {
            erlog!("{line}");
        }
        if lift {
            crate::inputs::set_mask(gameplay_mask);
        }
        if press {
            let how = crate::inputs::press_for_er(crate::inputs::XI_BACK, crate::inputs::SCAN_G);
            erlog!("map: View pressed for the game by the guard ({})", how.unwrap_or("NO WAY: neither a pad it reads nor its window in front"));
        }
        if control.flag("log.map") != Some(false) {
            for line in self.requests.frame(view_raw, sp_press || press, facts, dt) {
                erlog!("{line}");
            }
        } else {
            self.requests.frame(view_raw, sp_press || press, facts, dt);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 1.0 / 60.0;

    fn field() -> Facts {
        Facts { foreground: true, hud_state: 3, er_reads_pad: true, ladder: -1, ..Facts::default() }
    }

    #[test]
    fn a_view_press_that_opens_the_map_and_one_that_does_not() {
        let mut r = Requests::default();
        let lines = r.frame(true, false, field(), DT);
        assert!(lines[0].starts_with("map: request #1 (View in the field)"), "{lines:?}");
        assert!(r.frame(true, false, field(), DT).is_empty());
        let up = Facts { hud_state: 0, ..field() };
        let lines = r.frame(false, false, up, DT);
        assert!(lines[0].contains("#1: THE MAP IS UP"), "{lines:?}");
        // closing it with View is no request
        assert!(r.frame(true, false, up, DT).is_empty());
        assert!(r.frame(false, false, field(), DT).is_empty());
        // THE USER'S CASE: the window in the background; the save point presses four times, nothing opens
        let back = Facts { foreground: false, ..field() };
        let mut all = Vec::new();
        for k in 0..400 {
            all.extend(r.frame(false, k % 72 == 0 && k < 288, back, DT));
        }
        assert_eq!(all.len(), 2, "{all:?}");
        assert!(all[0].contains("request #2 (the save point's") && all[0].contains("NOT IN FRONT"), "{all:?}");
        assert!(all[1].contains("#2: THE MAP DID NOT OPEN (4 press(es)") && all[1].contains("THE GAME'S WINDOW IS NOT IN FRONT"), "{all:?}");
    }

    #[test]
    fn the_pad_is_gated_while_the_window_is_out_of_front() {
        let mut f = Focus::default();
        assert_eq!(f.frame(true, true, DT), (true, None));
        // a flicker
        assert_eq!(f.frame(false, true, DT), (true, None));
        assert_eq!(f.frame(true, true, DT), (true, None));
        let mut edges = Vec::new();
        for _ in 0..60 {
            let (_, e) = f.frame(false, true, DT);
            edges.extend(e);
        }
        assert_eq!(edges, vec![true]);
        assert!(!f.frame(false, true, DT).0);
        assert_eq!(f.frame(true, true, DT), (true, Some(false)));
        // the key off: never gated
        let mut off = Focus::default();
        assert!((0..600).all(|_| off.frame(false, false, DT).0));
    }

    #[test]
    fn the_guard_frees_view_only_from_a_stuck_state_without_a_menu() {
        // a menu of ours is up for a minute: View stays ours, a press does nothing
        let mut g = Guard::default();
        let menu = Facts { view_hidden: true, menu_ours: true, ..field() };
        assert!((0..3600).all(|k| g.frame(k % 600 == 0, menu, true, DT) == (false, false, None)));
        // a cover that never ends: pressed before 10 s = nothing; after = the press and the lift
        let mut g = Guard::default();
        let stuck = Facts { view_hidden: true, cover: true, ..field() };
        assert_eq!(g.frame(true, stuck, true, DT).0, false);
        for _ in 0..600 {
            g.frame(false, stuck, true, DT);
        }
        let (press, lift, line) = g.frame(true, stuck, true, DT);
        assert!(press && lift && line.unwrap().starts_with("!!! map:"));
        assert_eq!(g.frame(false, stuck, true, DT), (false, true, None));
        // it ends by itself
        let (press, lift, line) = g.frame(false, field(), true, DT);
        assert!(!press && !lift && line.is_some());
        // the key off
        let mut g = Guard::default();
        for _ in 0..1200 {
            g.frame(false, stuck, false, DT);
        }
        assert_eq!(g.frame(true, stuck, false, DT), (false, false, None));
    }
}
