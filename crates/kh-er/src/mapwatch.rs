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
//! - (MAP-GATE, the user 2026-11-08: "using a ladder disables anything to do with the map, fix this") THE GAME'S OWN GATE
//!   REFUSED: his log had every map request after his ladders fail with "nothing of ours stood in the way". A request the
//!   game has not answered `Requests::GAME_FIRST` s after its latest press (counted while the game takes input) is given
//!   the game's own map request (mapgate.rs: the byte its View handler sets; the map opens through its own code), once
//!   more after `DELIVERY_WITHIN` s, then taken back; an action-button press left waiting (one of the gate's parts) is
//!   ended as the game ends one it carried out. Never past a cutscene, a dead body or a busy menu of the game's. Every
//!   failure names the parts of the game's gate that refused. `map.direct = 0`: the request is never made by us.
//! - (MAP-GATE) A request made while the game does not take input yet (the first moments after a load: no pad read, the
//!   world still) waits up to `WAIT_MAX` s for it instead of failing. A MapWatch made at a load counts no earlier press
//!   of the save point's (each load used to log a phantom "request #1" that failed).
//! - (MAP-NOW, the user 2026-11-10: "pressing the map button there is a slight delay before the map actually shows up")
//!   THE 0.6 s WAIT IS GONE. His log (DLL 41a2f63d5d10): every View press was "the game did not take the press in 0.6 s:
//!   ITS OWN MAP REQUEST IS MADE ... THE MAP IS UP after 0.60 s", always after a ladder (the LADDER2 leap) and with every
//!   part of the gate we read clear: a part of the game's map gate 0x140e2ce90 that we did not read stays shut after the
//!   leap (mapgate.rs now logs the three readable ones). Now the request is ARMED ON THE PRESS'S OWN FRAME when every part
//!   we read allows it (the game's menus, its cutscene, the body alive, its menu flags, no use-item cancel, not on a ladder;
//!   a waiting action-button press is ended first), and the popup menu's update writes the map byte `+0x122` right before
//!   the game's own update (mapgate.rs: never twice for one press, never `+0x121`). A refusal of those parts keeps the map
//!   shut as in Elden Ring ("THE GAME'S OWN GATE KEEPS ITS MAP SHUT NOW"). Sources: View on any pad (XInput, Steam Input),
//!   the game's own map key G as its own keyboard read sees it (dinput_mask.rs), the save point's Select an Area.

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
    /// (MAP-GATE) A pad is connected (with none, the game's pad read is not waited for).
    pub pad_connected: bool,
    /// (MAP-GATE) A cutscene runs (ours or the game's).
    pub cutscene: bool,
    /// (MAP-GATE) The party leader is alive.
    pub leader_alive: bool,
}

impl Facts {
    fn gameplay(&self) -> bool {
        self.hud_state == 3
    }

    /// (MAP-GATE) The game takes input in the field again: its interface at gameplay, its window in front, its world
    /// running, no menu or cover of ours, no cutscene, the leader alive, and its pad read when a pad is connected (just
    /// after a load the game reads no pad for a moment: a request made then waits for this).
    pub fn ready(&self) -> bool {
        self.gameplay() && self.foreground && !self.menu_ours && !self.cover && !self.world_still && !self.cutscene && self.leader_alive && (self.er_reads_pad || !self.pad_connected)
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
    /// (MAP-NOW) the game's own map key on the keyboard (G), as the game's own keyboard read saw it (dinput_mask.rs)
    Key,
    SavePoint,
}

impl Source {
    fn words(self) -> &'static str {
        match self {
            Source::View => "View in the field",
            Source::Key => "G on the keyboard in the field",
            Source::SavePoint => "the save point's Select an Area: View pressed for the game",
        }
    }
}

/// (MAP-NOW) The map presses of one frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Presses {
    /// View physically down on the real pad (any pad through XInput, Steam Input's too).
    pub view: bool,
    /// The game's own map key (G) down in the game's own keyboard read.
    pub key: bool,
    /// The save point's map flow pressed View for the game this frame.
    pub save_point: bool,
}

#[derive(Clone, Copy, Debug)]
struct Pending {
    n: u32,
    source: Source,
    /// Seconds since the latest press, counted only while the game takes input (`Facts::ready`).
    age: f32,
    presses: u32,
    at_press: Facts,
    /// Frames of the window in which the game's window was not in front.
    background: u32,
    frames: u32,
    /// (MAP-GATE) Seconds the game has not taken input since the press (a load's first moments), and whether it was said.
    waited: f32,
    told_wait: bool,
    /// (MAP-GATE) The game's own map request made by us so far, and the seconds since the latest.
    delivered: u8,
    since_delivery: f32,
    /// (MAP-NOW) The refusal of the game's gate was said.
    told_refusal: bool,
}

/// (MAP-GATE) What MapWatch does for a request this frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Order {
    /// Make the game's own map request (mapgate.rs `arm`: written at the popup menu's next update).
    pub deliver: bool,
    /// End the action-button press that waits (mapgate.rs `consume_action_press`).
    pub consume_press: bool,
    /// Take the request back (the map did not come).
    pub cancel: bool,
}

/// The requests and their outcomes (pure; returns the log lines and what to do).
#[derive(Debug, Default)]
pub struct Requests {
    count: u32,
    pending: Option<Pending>,
    view_before: bool,
    key_before: bool,
}

impl Requests {
    /// The game's interface must leave gameplay this soon after a press (when the map may not be given).
    pub const WITHIN: f32 = 2.0;
    /// (MAP-NOW) The game's own map request is made ON THE PRESS'S FRAME when the game's gate allows it; a press the gate
    /// refuses on that frame still gets it if the gate opens this soon after (a few frames). Was (MAP-GATE) a 0.6 s wait
    /// for the game's own route before every request: the user's delay (2026-11-10).
    pub const TAKE_WINDOW: f32 = 0.15;
    /// (MAP-GATE) After our request the map must be up this soon; then one more request, then the request is taken back.
    pub const DELIVERY_WITHIN: f32 = 1.0;
    pub const DELIVERIES: u8 = 2;
    /// (MAP-GATE) A request waits this long at most for the game to take input again.
    pub const WAIT_MAX: f32 = 8.0;

    /// One frame. `press`: the map presses of this frame; `gate`: the game's own map gate as it holds it; `direct`: our
    /// request may be made.
    pub fn frame(&mut self, press: Presses, facts: Facts, gate: crate::mapgate::Gate, direct: bool, dt: f32) -> (Vec<String>, Order) {
        let mut lines = Vec::new();
        let mut order = Order::default();
        let edge = press.view && !self.view_before;
        self.view_before = press.view;
        let key_edge = press.key && !self.key_before;
        self.key_before = press.key;
        // (a View press while the game's map / a menu of its is up closes it: no request)
        let field_press = (edge || key_edge) && facts.gameplay() && !facts.menu_ours; // (View in a menu of ours is that menu's)
        let pressed = field_press || press.save_point;
        if pressed {
            let source = if press.save_point { Source::SavePoint } else if edge { Source::View } else { Source::Key };
            match self.pending.as_mut() {
                Some(p) if p.source == source || press.save_point => {
                    // another press of the same request (the save point's retries; a second View inside the window)
                    p.presses += 1;
                    p.age = 0.0;
                    p.source = source;
                }
                _ => {
                    self.count += 1;
                    self.pending = Some(Pending { n: self.count, source, age: 0.0, presses: 1, at_press: facts, background: 0, frames: 0, waited: 0.0, told_wait: false, delivered: 0, since_delivery: 0.0, told_refusal: false });
                    let obstacles = facts.obstacles();
                    lines.push(format!(
                        "map: request #{} ({}): the game's interface hud_state {}, its window {}{}",
                        self.count,
                        source.words(),
                        facts.hud_state,
                        if facts.foreground { "in front" } else { "NOT IN FRONT" },
                        if obstacles.is_empty() { String::new() } else { format!(" | in the way: {}", obstacles.join("; ")) },
                    ));
                }
            }
        }
        let Some(p) = self.pending.as_mut() else { return (lines, order) };
        if !pressed {
            p.frames += 1;
            p.background += u32::from(!facts.foreground);
            if !facts.gameplay() && facts.hud_state != 255 {
                let how = if p.delivered > 0 { format!("; the game's own map request made by us {} time(s)", p.delivered) } else { "; the game's own route, nothing of ours".to_string() };
                lines.push(format!("map: request #{}: THE MAP IS UP ({} press(es); the game's interface hud_state {} after {:.2} s{how})", p.n, p.presses, facts.hud_state, p.age + p.since_delivery + dt));
                self.pending = None;
                return (lines, order);
            }
        }
        let ready = facts.ready();
        let allowed = ready && facts.ladder < 0 && gate.map_may_open();
        let fail = |p: &Pending, why_more: Option<String>| {
            let mut why = p.at_press.obstacles();
            if p.background > 0 && p.at_press.foreground {
                why.push(format!("the game's window was not in front in {} of {} frame(s) after the press", p.background, p.frames));
            }
            why.extend(why_more);
            format!(
                "!!! map: request #{}: THE MAP DID NOT OPEN ({} press(es), {:.1} s after the last): {} | the game's own gate: {}",
                p.n,
                p.presses,
                p.age + p.waited + p.since_delivery,
                if why.is_empty() { "nothing of ours stood in the way".to_string() } else { why.join("; ") },
                gate.text(),
            )
        };
        if p.delivered > 0 && !pressed {
            p.since_delivery += dt;
            if p.since_delivery >= Self::DELIVERY_WITHIN {
                if p.delivered < Self::DELIVERIES && allowed && direct {
                    p.delivered += 1;
                    p.since_delivery = 0.0;
                    order = Order { deliver: true, consume_press: gate.action_press_waiting == Some(true), cancel: false };
                    lines.push(format!("map: request #{}: the map is not up {:.1} s after the game's own map request: made once more | the game's own gate: {}", p.n, Self::DELIVERY_WITHIN, gate.text()));
                } else {
                    order.cancel = true;
                    lines.push(fail(p, Some(format!("the game's own map request made {} time(s) did not bring it up either (taken back)", p.delivered))));
                    self.pending = None;
                }
            }
            return (lines, order);
        }
        if !ready {
            p.waited += dt;
            if !p.told_wait {
                p.told_wait = true;
                lines.push(format!("map: request #{}: the game does not take input yet (after a load, a menu or a cover): the request waits up to {:.0} s for it", p.n, Self::WAIT_MAX));
            }
            if p.waited > Self::WAIT_MAX {
                lines.push(fail(p, Some(format!("the game did not take input again within {:.0} s", Self::WAIT_MAX))));
                self.pending = None;
            }
            return (lines, order);
        }
        if p.delivered == 0 && direct && p.age < Self::TAKE_WINDOW && allowed {
            p.delivered = 1;
            p.since_delivery = 0.0;
            order = Order { deliver: true, consume_press: gate.action_press_waiting == Some(true), cancel: false };
            lines.push(format!(
                "map: request #{}: THE GAME'S OWN MAP REQUEST IS MADE AT ONCE ({:.2} s after the press{}; the map byte +0x122 at the popup menu's next update){} | the game's own gate: {}",
                p.n,
                p.age,
                if p.told_wait { format!(", after {:.1} s waiting for the game to take input", p.waited) } else { String::new() },
                if order.consume_press { "; the action-button press that waits is ended as the game ends one it carried out" } else { "" },
                gate.text(),
            ));
            return (lines, order);
        }
        if p.delivered == 0 && !p.told_refusal && (!allowed || !direct) {
            p.told_refusal = true;
            let why = if !direct {
                "map.direct = 0: the game's own request is not made by us".to_string()
            } else if facts.ladder >= 0 {
                format!("the body is on a ladder (state {} {})", facts.ladder, crate::ladder::state_name(facts.ladder))
            } else {
                gate.refusals().join("; ")
            };
            lines.push(format!("map: request #{}: THE GAME'S OWN GATE KEEPS ITS MAP SHUT NOW ({why}): no request of ours (as in Elden Ring: press again once it allows)", p.n));
        }
        p.age += dt;
        if p.delivered == 0 && p.age > Self::WITHIN {
            let why = if !direct { Some("map.direct = 0: the game's own request is not made".to_string()) } else if !allowed { Some("the game rightly keeps its map shut now".to_string()) } else { None };
            lines.push(fail(p, why));
            self.pending = None;
        }
        (lines, order)
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
    /// The save point's press count as last seen (None: not yet seen. A MapWatch made at a load takes the count as it
    /// stands: the presses made before it are no request of its own; until 2026-11-08 every load logged one, "request #1
    /// (the save point's Select an Area)" that failed with the body at animation -1 and no pad read).
    sp_presses: Option<u32>,
}

/// (MAP-GATE) The save point's map flow pressed this frame: the count moved since it was last seen (a first sight is no press).
pub fn save_point_pressed(seen: &mut Option<u32>, now: u32) -> bool {
    let pressed = seen.is_some_and(|s| s != now);
    *seen = Some(now);
    pressed
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
    /// computed for gameplay (before a cover); `facts` without the mask (read here); `player`: the body (the game's map
    /// gate is read from it while a request is open).
    pub fn frame(&mut self, control: &crate::control::Control, view_raw: bool, gameplay_mask: u32, mut facts: Facts, player: &eldenring::cs::PlayerIns, dt: f32) {
        facts.view_hidden = crate::inputs::current_mask() & u32::from(crate::inputs::XI_BACK) != 0;
        facts.foreground = crate::inputs::game_in_foreground();
        facts.er_reads_pad = crate::inputs::er_reads_pad();
        facts.pad_connected = (0..4).any(|i| crate::inputs::xinput_state(i).is_some());
        let sp_press = save_point_pressed(&mut self.sp_presses, crate::travel::save_point_presses());
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
        // (MAP-NOW) the game's own map key on the keyboard, as the game's own keyboard read saw it (dinput_mask.rs)
        let presses = Presses { view: view_raw, key: crate::dinput_mask::game_map_key_down(), save_point: sp_press || press };
        // (MAP-GATE) the game's own gate, read while a request is open or one is pressed (`map.direct = 0`: its request is never made)
        let any_press = presses.view || presses.key || presses.save_point;
        let gate = if self.requests.pending.is_some() || any_press { crate::mapgate::read(player) } else { crate::mapgate::Gate::default() };
        let open = self.requests.pending.map(|p| p.n);
        let (lines, order) = self.requests.frame(presses, facts, gate, control.flag("map.direct") != Some(false), dt);
        let log = control.flag("log.map") != Some(false);
        if log {
            for line in lines {
                erlog!("{line}");
            }
            // (MAP-NOW) what the popup menu's update did with the request armed for the press (mapgate.rs)
            if let Some(step) = crate::mapgate::take_outcome() {
                let n = self.requests.pending.map(|p| p.n).or(open).unwrap_or(0);
                erlog!("map: request #{n}: at the popup menu's update: {}", match step {
                    crate::mapgate::Step::Write => "OUR MAP REQUEST WRITTEN (+0x122 = 1, the game's own update opens 02_120_WorldMap now)",
                    crate::mapgate::Step::GameHas => "the game's own View route had already set +0x122: it took the press itself (nothing of ours written)",
                    crate::mapgate::Step::MapUp => "the map's element was already created: nothing written",
                    crate::mapgate::Step::Drop => "A SECOND MAP REQUEST FOR THE SAME PRESS WAS PUT BACK (+0x122 = 0): one map, never two",
                    crate::mapgate::Step::Nothing => "nothing",
                });
            }
        }
        if order.consume_press && crate::mapgate::consume_action_press() && log {
            erlog!("map: the action-button press that waited was ended (CSActionButtonMan +0x80, as the game ends a press it carried out)");
        }
        if order.deliver && !crate::mapgate::arm() {
            erlog!("!!! map: the game's own map request could not be made (no popup menu object)");
        }
        if order.cancel && crate::mapgate::cancel_request() && log {
            erlog!("map: the game's own map request was taken back (it must not open the map later by itself)");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 1.0 / 60.0;

    fn field() -> Facts {
        Facts { foreground: true, hud_state: 3, er_reads_pad: true, pad_connected: true, leader_alive: true, ladder: -1, ..Facts::default() }
    }

    fn open_gate() -> crate::mapgate::Gate {
        crate::mapgate::Gate { menu_busy: Some(false), menu_job: Some(false), action_press_waiting: Some(false), cutscene: Some(false), start_inputs_off: Some(false), dead: Some(false), item_cancel: Some(false), ui_element_2: Some(false), modifier_bit: Some(false), event_state: Some(0) }
    }

    const VIEW: Presses = Presses { view: true, key: false, save_point: false };
    const KEY: Presses = Presses { view: false, key: true, save_point: false };
    const SAVE_POINT: Presses = Presses { view: false, key: false, save_point: true };
    const NONE: Presses = Presses { view: false, key: false, save_point: false };

    /// Frames until the first order to deliver or cancel (None: none in `n` frames); every line is collected.
    fn run(r: &mut Requests, facts: Facts, gate: crate::mapgate::Gate, n: usize, lines: &mut Vec<String>) -> Option<(usize, Order)> {
        run_with(r, facts, gate, true, n, lines)
    }

    fn run_with(r: &mut Requests, facts: Facts, gate: crate::mapgate::Gate, direct: bool, n: usize, lines: &mut Vec<String>) -> Option<(usize, Order)> {
        for k in 0..n {
            let (l, o) = r.frame(NONE, facts, gate, direct, DT);
            lines.extend(l);
            if o.deliver || o.cancel {
                return Some((k, o));
            }
        }
        None
    }

    /// (MAP-NOW) THE USER'S LOG (2026-11-10, DLL 41a2f63d5d10, er\logs\kh-er.log 54.357 s): "map: request #1 (View in the
    /// field): the game's interface hud_state 3, its window in front", every part of the gate read clear ("nothing of the
    /// parts read refuses"), the game's own route never took the press; DEPLOY8-10 made the request only "the game did not
    /// take the press in 0.6 s" later (55.873 s with three presses). Now the request is made ON THE PRESS'S OWN FRAME.
    /// (Fails on the old code: its first order came 36 frames after the press.)
    #[test]
    fn the_users_log_a_view_press_gets_the_map_request_on_its_own_frame() {
        let mut r = Requests::default();
        let (lines, order) = r.frame(VIEW, field(), open_gate(), true, DT);
        assert!(lines[0].starts_with("map: request #1 (View in the field): the game's interface hud_state 3, its window in front"), "{lines:?}");
        assert!(order.deliver && !order.consume_press && !order.cancel, "the request on the press's own frame: {order:?}");
        assert!(lines.iter().any(|l| l.contains("#1: THE GAME'S OWN MAP REQUEST IS MADE AT ONCE (0.00 s after the press")), "{lines:?}");
        assert!(lines.iter().all(|l| !l.contains("0.6 s")), "no 0.6 s wait: {lines:?}");
        // the popup update opens it: the next frame the interface has left gameplay
        let (lines, order) = r.frame(VIEW, Facts { hud_state: 0, ..field() }, open_gate(), true, DT);
        assert!(lines[0].contains("#1: THE MAP IS UP (1 press(es); the game's interface hud_state 0 after 0.02 s; the game's own map request made by us 1 time(s))"), "{lines:?}");
        assert_eq!(order, Order::default());
        // two more presses before the map came (the user pressed three times, 54.357 - 55.873 s): one request, no second one
        let mut r = Requests::default();
        let mut delivered = 0;
        for k in 0..55 {
            let press = if k % 30 < 3 { VIEW } else { NONE };
            let (_, o) = r.frame(press, field(), open_gate(), true, DT);
            delivered += usize::from(o.deliver);
        }
        assert_eq!(delivered, 1, "two presses before the map came: one request");
    }

    /// (MAP-NOW) The keyboard's map key (G, as the game's own keyboard read sees it) and the save point's press: the same.
    #[test]
    fn the_keyboard_and_the_save_point_get_it_at_once_too() {
        for (press, words) in [(KEY, "(G on the keyboard in the field)"), (SAVE_POINT, "(the save point's Select an Area: View pressed for the game)")] {
            let mut r = Requests::default();
            let (lines, order) = r.frame(press, field(), open_gate(), true, DT);
            assert!(lines[0].starts_with("map: request #1 ") && lines[0].contains(words), "{lines:?}");
            assert!(order.deliver, "{words}: {lines:?}");
        }
        // a key held down is one press
        let mut r = Requests::default();
        let mut n = 0;
        for _ in 0..30 {
            n += usize::from(r.frame(KEY, field(), open_gate(), true, DT).1.deliver);
        }
        assert_eq!(n, 1);
    }

    /// (MAP-GATE kept) A waiting action-button press (the ladder's lesson) is ended and the request made, at once.
    #[test]
    fn a_waiting_press_is_ended_and_the_request_made_at_once() {
        let mut r = Requests::default();
        let stale = crate::mapgate::Gate { action_press_waiting: Some(true), ..open_gate() };
        let (lines, order) = r.frame(VIEW, field(), stale, true, DT);
        assert!(order.deliver && order.consume_press, "{order:?}");
        assert!(lines.iter().any(|l| l.contains("MADE AT ONCE") && l.contains("ACTION-BUTTON PRESS IS WAITING") && l.contains("the action-button press that waits is ended")), "{lines:?}");
    }

    /// (MAP-NOW) What keeps the map shut in Elden Ring keeps it shut: the game's gate refusing (its menu flags, a use-item
    /// cancel, a cutscene, the body dead, a menu busy), the body on a ladder, our key off. Nothing is forced, the refusal is
    /// said once, and the request ends after WITHIN. (Fails on the old code: it went past the menu flags and the item cancel,
    /// and never looked at the ladder.)
    #[test]
    fn nothing_is_forced_past_the_games_own_gate() {
        let shut = [
            (crate::mapgate::Gate { start_inputs_off: Some(true), ..open_gate() }, field(), true, "the body's menu flags hold the menus shut"),
            (crate::mapgate::Gate { item_cancel: Some(true), ..open_gate() }, field(), true, "a use-item cancel is waiting"),
            (crate::mapgate::Gate { cutscene: Some(true), ..open_gate() }, field(), true, "a real-time cutscene runs"),
            (crate::mapgate::Gate { dead: Some(true), ..open_gate() }, field(), true, "the body is dead"),
            (crate::mapgate::Gate { menu_busy: Some(true), ..open_gate() }, field(), true, "a menu of the game's is busy"),
            (crate::mapgate::Gate { menu_job: Some(true), ..open_gate() }, field(), true, "a menu job of the game's is pending"),
            (open_gate(), Facts { ladder: 2, ..field() }, true, "the body is on a ladder (state 2"),
            (open_gate(), field(), false, "map.direct = 0"),
        ];
        for (gate, facts, direct, why) in shut {
            let mut r = Requests::default();
            let (mut lines, order) = r.frame(VIEW, facts, gate, direct, DT);
            assert!(!order.deliver, "{why}");
            assert!(run_with(&mut r, facts, gate, direct, 200, &mut lines).is_none(), "{why}");
            let told: Vec<&String> = lines.iter().filter(|l| l.contains("THE GAME'S OWN GATE KEEPS ITS MAP SHUT NOW")).collect();
            assert!(told.len() == 1 && told[0].contains(why), "{why}: {lines:?}");
            assert!(lines.iter().any(|l| l.contains("THE MAP DID NOT OPEN")), "{lines:?}");
        }
        // the logged-only parts never decide (one of them is what stays shut after the LADDER2 leap)
        let logged = crate::mapgate::Gate { ui_element_2: Some(true), modifier_bit: Some(true), event_state: Some(1), ..open_gate() };
        let (lines, order) = Requests::default().frame(VIEW, field(), logged, true, DT);
        assert!(order.deliver && lines.iter().any(|l| l.contains("ONE OF THESE WOULD KEEP THE GAME'S OWN ROUTE SHUT")), "{lines:?}");
        // our cutscene / a dead leader / a menu of ours: the request waits (not ready), nothing is given
        for f in [Facts { cutscene: true, ..field() }, Facts { leader_alive: false, ..field() }, Facts { menu_ours: true, ..field() }] {
            let mut r = Requests::default();
            r.frame(SAVE_POINT, f, open_gate(), true, DT);
            assert!(run(&mut r, f, open_gate(), 300, &mut Vec::new()).is_none(), "{f:?}");
        }
    }

    /// (MAP-NOW) A gate that opens a few frames after the press (inside TAKE_WINDOW) still gets the request; later it does not
    /// (Elden Ring takes its own press on its edge: no press is held for seconds).
    #[test]
    fn a_gate_that_opens_inside_the_take_window_and_one_that_opens_after() {
        let flags = crate::mapgate::Gate { start_inputs_off: Some(true), ..open_gate() };
        let mut r = Requests::default();
        r.frame(VIEW, field(), flags, true, DT);
        for _ in 0..4 {
            assert!(!r.frame(NONE, field(), flags, true, DT).1.deliver);
        }
        assert!(r.frame(NONE, field(), open_gate(), true, DT).1.deliver, "5 frames after the press: inside the window");
        let mut r = Requests::default();
        r.frame(VIEW, field(), flags, true, DT);
        for _ in 0..12 {
            r.frame(NONE, field(), flags, true, DT);
        }
        assert!(run(&mut r, field(), open_gate(), 200, &mut Vec::new()).is_none(), "after the window: no late map");
    }

    /// The request made twice brings nothing: it is taken back (it must not open the map later by itself), with the gate named.
    #[test]
    fn a_request_that_brings_nothing_is_taken_back() {
        let mut r = Requests::default();
        assert!(r.frame(VIEW, field(), open_gate(), true, DT).1.deliver);
        let mut lines = Vec::new();
        let (k, o) = run(&mut r, field(), open_gate(), 120, &mut lines).expect("again");
        assert!(o.deliver && ((k + 1) as f32 * DT - Requests::DELIVERY_WITHIN).abs() < 0.03, "{k}");
        let (_, o) = run(&mut r, field(), open_gate(), 120, &mut lines).expect("back");
        assert!(o.cancel && !o.deliver);
        assert!(lines.last().is_some_and(|l| l.contains("THE MAP DID NOT OPEN") && l.contains("made 2 time(s)") && l.contains("the game's own gate:")), "{lines:?}");
        assert!(r.frame(NONE, field(), open_gate(), true, DT) == (Vec::new(), Order::default()));
    }

    /// KIND B (every load): a request made just after a load, while the game reads no pad yet, waits for it and is then
    /// given AT ONCE when it takes input; a MapWatch made at a load counts no press of the save point's from before it.
    #[test]
    fn an_early_request_waits_for_the_game_and_a_load_makes_no_request() {
        let mut r = Requests::default();
        let early = Facts { er_reads_pad: false, body_anim: -1, ..field() };
        let (mut lines, order) = r.frame(SAVE_POINT, early, open_gate(), true, DT);
        assert!(!order.deliver);
        assert!(run(&mut r, early, open_gate(), 240, &mut lines).is_none(), "not while the game reads no pad");
        assert!(lines.iter().any(|l| l.contains("does not take input yet")), "{lines:?}");
        let (k, o) = run(&mut r, field(), open_gate(), 120, &mut lines).expect("given once it reads the pad");
        assert!(o.deliver && k == 0, "the first frame it takes input: {k}");
        assert!(lines.iter().any(|l| l.contains("MADE AT ONCE") && l.contains("after 4.0 s waiting")), "{lines:?}");
        // no pad connected (keyboard): the pad read is not waited for
        assert!(Facts { er_reads_pad: false, pad_connected: false, ..field() }.ready());
        // the count as a new MapWatch first sees it is no press; a change after is
        let mut seen = None;
        assert!(!save_point_pressed(&mut seen, 7));
        assert!(!save_point_pressed(&mut seen, 7));
        assert!(save_point_pressed(&mut seen, 8));
    }

    /// THE USER'S CASE (2026-10-13): the window in the background; the save point presses four times: the request waits for
    /// the game to take input again, and is given up after WAIT_MAX with the window named; a View press inside a game menu
    /// (closing it) is no request.
    #[test]
    fn the_window_in_the_background_and_view_in_a_menu() {
        let mut r = Requests::default();
        let up = Facts { hud_state: 0, ..field() };
        assert!(r.frame(VIEW, up, open_gate(), true, DT).0.is_empty());
        assert!(r.frame(NONE, field(), open_gate(), true, DT).0.is_empty());
        let back = Facts { foreground: false, ..field() };
        let mut all = Vec::new();
        for k in 0..700 {
            let press = if k % 72 == 0 && k < 288 { SAVE_POINT } else { NONE };
            let (l, o) = r.frame(press, back, open_gate(), true, DT);
            assert!(!o.deliver, "never given while the window is out of front");
            all.extend(l);
        }
        assert_eq!(all.len(), 3, "{all:?}");
        assert!(all[0].contains("request #1 (the save point's") && all[0].contains("NOT IN FRONT"), "{all:?}");
        assert!(all[1].contains("waits up to 8 s"), "{all:?}");
        assert!(all[2].contains("#1: THE MAP DID NOT OPEN (4 press(es)") && all[2].contains("THE GAME'S WINDOW IS NOT IN FRONT"), "{all:?}");
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
        assert!(!g.frame(true, stuck, true, DT).0);
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
