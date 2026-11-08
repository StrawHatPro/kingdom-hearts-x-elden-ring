//! (GF-SENSOR) R6: AN ENEMY'S STANCE FORCED TO BREAK, BY LADDER (build/sora_tmp/party/GOOFY/SENSOR_NOTES.md has what each step did
//! in game). [`begin`] asks for it; [`frame`] runs the steps:
//! 1. (COMBAT-FIX, THE USER 2026-11-06: "enemies who's stagger is broken by Goofy need to play the animation they play when their
//!    stance meter is broken") ELDEN RING'S OWN STANCE BREAK, THROUGH ITS DAMAGE: one bullet of the player's with more poise damage
//!    than any body has and no `dmgLevel` ([`break_attack`], `ErTargets::ask_stance_break`), the armour left as it is, so the
//!    game's own damage code empties the stance meter and plays its own break: the animation this character plays when the
//!    Tarnished breaks it, its critical window, its meter refilled, its recovery. Generic: no animation of ours is named;
//! 2. the game's own event `W_SABreak` (chranim::fire): the kneel plays at once where the character has one (GF-SENSOR's step 1,
//!    proven on c4070 / c4311 / c4351 / c2130), for a body the bullet broke not (no bullet route, a miss, 1 HP left);
//! 3. nothing played: its own animation frozen with its AI off, for the window.
//!    (COMBAT-FIX: the old step "its armour set to 0, then a dmgLevel-3 bullet" is gone: a meter already at 0 never crosses the
//!    break, so the game played the level's stagger (zpose's 8050), never its kneel.)
//!
//! The kneel is confirmed by `puppet::damage_anim`. (COMBAT-FIX) ITS END IS THE GAME'S: when the window runs out with no slam the
//! kneel plays on to its own recovery (no `W_Idle`; hits take HP and start no reaction over it until it gets up); only the
//! Breaker's slam cuts it with `W_Idle` (goofy_breaker.rs `slam`: "the kneel must end for the slam's own reaction to play on
//! it"). Never in a no-death state or
//! a cutscene (`zdefer::no_dead_of`): the owner's rule. The chain cap and bossguard are GF-BREAK's: this module only runs the
//! game's side. `atk.break = n,seconds` (test key, sensorlab.rs) asks it of the n-th nearest hostile.
//!
//! (GF-BREAK) THE STAGGER BOOK'S SIDE (`ErTargets::stagger_ask`, targets.rs): [`begin_grounded`] for a body a KH3 reaction may hold
//! in the air, [`hold`] while a Breaker runs (a kneel shorter than the hold plays slower), [`release`] at the slam or the window's
//! end. While it kneels a hit starts no reaction over it ([`kneeling`], puppet.rs).

use std::sync::Mutex;

use eldenring::cs::{ChrIns, FieldInsHandle};
use kh_core::TargetId;

use crate::chranim::AnimNow;
use crate::erlog;
use crate::targets::ErTargets;

/// How it went, by [`state`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Phase {
    /// Asked; nothing seen yet.
    Asking,
    /// Kneeling (the ladder step that worked, seconds left of the window).
    Down { step: u8, left: f32 },
    /// Frozen in place by step 3 (seconds left of the window).
    Frozen { left: f32 },
    /// Refused: a no-death state, or the character is gone.
    Refused,
    /// The window ran out; the enemy is its own again.
    Over,
}

/// Seconds a step waits for the kneel before the next one is tried.
const STEP_WAIT: f32 = 0.3;
/// The kneels are x8700..x8799 of a set (MEASURED 3008700 on c4311, 5 s on Margit).
const KNEEL: std::ops::Range<i32> = 8700..8800;
/// (GF-BREAK) The kneel is not played slower than this for a hold.
const SLOWEST: f32 = 0.05;

/// (COMBAT-FIX) THE BULLET OF ELDEN RING'S OWN STANCE BREAK (step 1): poise damage past any body's meter (zpose's
/// `FORCE_POISE`), no `dmgLevel` (0: the reaction is the game's answer to its broken meter, as Sora's dmgLevel-0 bullets on
/// c3251 / c2130 MEASURED), the least attack value the bullet route takes (`landing`, else 1), no stamina.
pub fn break_attack(landing: u16) -> crate::bullet::Attack {
    crate::bullet::Attack { phys: landing.max(1), poise: crate::zpose::FORCE_POISE, stamina: 0, level: 0, ..crate::bullet::Attack::PLAIN }
}

struct Break {
    id: TargetId,
    handle: FieldInsHandle,
    window: f32,
    t: f32,
    step: u8,
    wait: f32,
    /// When the kneel was seen (`t`), and its animation.
    down: Option<(f32, i32)>,
    frozen: Option<bool>,
    /// (COMBAT-FIX) The window ran out with no slam: the kneel plays on to the game's own recovery.
    outro: bool,
    /// The kneel was cut with `W_Idle` this long ago (seconds); what plays is logged for a moment.
    cut: Option<(f32, i32)>,
    phase: Phase,
    /// (GF-BREAK) `release` asked: it ends at once.
    released: bool,
    /// (GF-BREAK) `hold` asked: the kneel must last this long more; and it plays slower now (taken by the step that slows it).
    hold: Option<f32>,
    /// (FINAL-FIX) A Breaker winds up on it (`hold` asked, until `release`): what Protect the slam reads. Apart from `hold`, which
    /// the next step takes for the slow-down: reading that one, the slam was "not held" one frame after the Breaker began.
    slam: bool,
    slowed: bool,
}

static BREAKS: Mutex<Vec<Break>> = Mutex::new(Vec::new());
/// (FINAL-FIX, FINAL-FIX launch 5: Donald's Ball Lightning killed a kneeling enemy in the Breaker's wind-up) THE SLAMS THAT WIND UP
/// NOW, the answer of [`slam_held`]: kept apart from BREAKS, so that a hit read while BREAKS is locked (`frame` steps the breaks with
/// the list locked; a frame task on another thread) still sees it. Written by [`hold`] / [`release`] / [`begin`] at once and
/// published from the list by `frame` after its step (the window's end); locked only for a copy or a look, never with BREAKS held.
static HELD: Mutex<Vec<TargetId>> = Mutex::new(Vec::new());

/// (FINAL-FIX) `id` winds up a Breaker now (true) or not.
fn mark_held(id: TargetId, on: bool) {
    let mut held = HELD.lock().unwrap_or_else(|e| e.into_inner());
    held.retain(|t| *t != id);
    if on {
        held.push(id);
    }
}

/// (FINAL-FIX) The rule of a held slam, on one break: `hold` asked, no `release`, the window not over.
fn holds(b: &Break) -> bool {
    b.slam && !b.released && !matches!(b.phase, Phase::Over | Phase::Refused)
}

/// (FINAL-FIX) [`HELD`] from the list as it stands (`frame`, after its step).
fn held_of(list: &[Break]) -> Vec<TargetId> {
    list.iter().filter(|b| holds(b)).map(|b| b.id).collect()
}

/// (GF-BREAK) Breaks that wait for the body to land from a KH3 reaction: (the target, seconds of the window left).
static WAITS: Mutex<Vec<(TargetId, f32)>> = Mutex::new(Vec::new());

/// Asks for `id`'s break, lasting `window` seconds (0 = the game's whole kneel). False: it is breaking already.
pub fn begin(targets: &ErTargets, id: TargetId, window: f32) -> bool {
    let Some(e) = targets.get(id) else { return false };
    let mut list = BREAKS.lock().unwrap_or_else(|e| e.into_inner());
    if list.iter().any(|b| b.id == id && !matches!(b.phase, Phase::Over | Phase::Refused)) {
        return false;
    }
    list.retain(|b| b.id != id);
    mark_held(id, false); // (FINAL-FIX) a new break holds no slam yet
    list.push(Break::new(id, e.handle, window));
    true
}

/// (GF-BREAK) As [`begin`], for the Stagger book: a body in the air of a KH3 reaction lands first, one on the ground leaves its
/// reaction (the kneel is the game's own and would not hold under a body we are placing).
pub fn begin_grounded(targets: &mut ErTargets, id: TargetId, window: f32) {
    if targets.puppets.airborne(id) {
        erlog!("stance: target {}: in the air of a reaction: the kneel waits for it to land", id.0);
        WAITS.lock().unwrap_or_else(|e| e.into_inner()).push((id, window));
        return;
    }
    targets.puppets.let_go(id);
    begin(targets, id, window);
}

/// Step 3's AI switch: the SDK extension is the party build's; without it the frozen pose stands alone.
#[cfg(feature = "party")]
fn ai_now(chr: &eldenring::cs::ChrIns) -> bool {
    crate::puppet::ai_switch::ai_on(chr).unwrap_or(true) // (GF-POLISH) through the pause of aipause.rs
}
#[cfg(not(feature = "party"))]
fn ai_now(_chr: &eldenring::cs::ChrIns) -> bool {
    true
}
#[cfg(feature = "party")]
fn set_ai(chr: &mut eldenring::cs::ChrIns, on: bool) {
    let _ = crate::sdkx::set_ai_on(chr, on);
}
#[cfg(not(feature = "party"))]
fn set_ai(_chr: &mut eldenring::cs::ChrIns, _on: bool) {}

/// (GF-BREAK) This animation is a kneel (x8700 .. x8799; plain 8702 on c4311, c4351).
pub fn is_kneel(anim: i32) -> bool {
    KNEEL.contains(&anim.rem_euclid(1_000_000))
}

/// (GF-BREAK) `id` kneels now and nothing has released it.
pub fn kneeling(id: TargetId) -> bool {
    // (`frame` holds the list while it runs; a bullet it asks for starts no hit of the party's)
    BREAKS.try_lock().is_ok_and(|list| list.iter().any(|b| b.id == id && b.kneels()))
}

/// (GF-BREAK) A Breaker runs: `id`'s break lasts at least `seconds` more. A kneel the game would end sooner plays slower.
pub fn hold(id: TargetId, seconds: f32) {
    let mut waits = WAITS.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(w) = waits.iter_mut().find(|w| w.0 == id) {
        w.1 = w.1.max(seconds);
    }
    drop(waits);
    let mut held = false;
    if let Some(b) = BREAKS.lock().unwrap_or_else(|e| e.into_inner()).iter_mut().find(|b| b.id == id && !matches!(b.phase, Phase::Over | Phase::Refused)) {
        held = b.hold_for(seconds); // (COMBAT-FIX) the per-break part, testable
    }
    // (FINAL-FIX) published at once, outside the BREAKS lock: the very next hit is capped
    if held {
        mark_held(id, true);
    }
    erlog!("stance: target {}: a Breaker winds up ({seconds:.2} s): Protect the slam {}", id.0, if held { "ON (every other party hit leaves it at least 1 HP)" } else { "NOT on: no active break of it" });
}

/// (PARTY-AI, THE USER 2026-11-03: "Protect the slam") A Breaker winds up on `id` now: [`hold`] was asked and neither the slam's
/// [`release`] nor the window's end has come.
/// (FINAL-FIX) Read from [`HELD`], kept by the break's own `slam` flag: the old rule read `hold`, which the next step takes for the
/// kneel's slow-down, so one frame after the Breaker began every party hit went through uncapped (FINAL-FIX launch 7: the
/// side hit and the second Ball Lightning on target 18); and never "not held" because BREAKS is locked (the old `try_lock`; the
/// hit went through uncapped).
pub fn slam_held(id: TargetId) -> bool {
    HELD.lock().unwrap_or_else(|e| e.into_inner()).contains(&id)
}

/// Where `id`'s break stands (None: never asked, or forgotten).
pub fn state(id: TargetId) -> Option<Phase> {
    BREAKS.lock().unwrap_or_else(|e| e.into_inner()).iter().find(|b| b.id == id).map(|b| b.phase)
}

/// Ends `id`'s break now. (COMBAT-FIX) The Breaker's slam (a `hold` was asked): the kneel is cut for the slam's reaction; the
/// window's end with no slam: the game's kneel plays on to its own recovery. The AI and the speed go back.
pub fn release(id: TargetId) {
    WAITS.lock().unwrap_or_else(|e| e.into_inner()).retain(|w| w.0 != id);
    if let Some(b) = BREAKS.lock().unwrap_or_else(|e| e.into_inner()).iter_mut().find(|b| b.id == id) {
        b.release_now(); // (COMBAT-FIX) the per-break part, testable
    }
    mark_held(id, false); // (FINAL-FIX) the slam's own release: from here on the hit is the slam's
}

/// (COMBAT-FIX) WHAT ONE STEP OF A BREAK READS OF THE CHARACTER AND ASKS OF IT: the game ([`GameBody`]) or a mock (the tests).
trait Body {
    /// Its HP (0 or less: dead).
    fn hp(&self) -> i32;
    /// Why it may not be forced now (`zdefer::no_dead_of`).
    fn no_dead(&self) -> Option<&'static str>;
    /// The newest animation of its TimeAct queue (`chranim::now`).
    fn now(&self) -> AnimNow;
    /// Its newest damage animation (`puppet::damage_anim`).
    fn damage_anim(&self) -> Option<AnimNow>;
    /// One of the game's behaviour events (`chranim::fire`).
    fn fire(&mut self, event: &str) -> bool;
    fn set_speed(&mut self, speed: f32);
    fn ai_on(&self) -> bool;
    fn set_ai(&mut self, on: bool);
    /// Elden Ring's own stance break asked through its damage (`ErTargets::ask_stance_break`): the bullet's row, or why none went.
    fn ask_break(&mut self) -> Result<u32, String>;
}

/// (COMBAT-FIX) The game's side of [`Body`]: the live character and the targets that fire its bullet.
struct GameBody<'a> {
    chr: &'a mut ChrIns,
    targets: &'a mut ErTargets,
    id: TargetId,
}

impl Body for GameBody<'_> {
    fn hp(&self) -> i32 {
        self.chr.modules.data.hp
    }
    fn no_dead(&self) -> Option<&'static str> {
        crate::zdefer::no_dead_of(self.chr)
    }
    fn now(&self) -> AnimNow {
        crate::chranim::now(self.chr)
    }
    fn damage_anim(&self) -> Option<AnimNow> {
        crate::puppet::damage_anim(self.chr)
    }
    fn fire(&mut self, event: &str) -> bool {
        crate::chranim::fire(self.chr, event)
    }
    fn set_speed(&mut self, speed: f32) {
        crate::chranim::set_speed(self.chr, speed);
    }
    fn ai_on(&self) -> bool {
        ai_now(self.chr)
    }
    fn set_ai(&mut self, on: bool) {
        set_ai(self.chr, on);
    }
    fn ask_break(&mut self) -> Result<u32, String> {
        self.targets.ask_stance_break(self.id)
    }
}

impl Break {
    fn new(id: TargetId, handle: FieldInsHandle, window: f32) -> Break {
        Break { id, handle, window, t: 0.0, step: 0, wait: 0.0, down: None, frozen: None, outro: false, cut: None, phase: Phase::Asking, released: false, hold: None, slam: false, slowed: false }
    }

    /// (GF-BREAK) It kneels and nothing has ended that. (COMBAT-FIX) Only the slam's release ends it early: a window that ran out
    /// leaves the game's kneel to its own recovery, and hits start no reaction over it until it gets up.
    fn kneels(&self) -> bool {
        matches!(self.phase, Phase::Down { .. }) && !(self.released && self.slam)
    }

    /// (COMBAT-FIX: [`hold`]'s part on one break) The kneel must last at least `seconds` more. True: the slam is held.
    fn hold_for(&mut self, seconds: f32) -> bool {
        let from = self.down.map_or(self.t, |d| d.0);
        self.window = self.window.max(self.t - from + seconds);
        self.hold = Some(seconds);
        self.slam = true;
        holds(self)
    }

    /// (COMBAT-FIX: [`release`]'s part on one break) The book's window ran out, or the Breaker's slam is about to land.
    fn release_now(&mut self) {
        self.window = self.window.min(self.t);
        self.released = true;
    }

    fn finish(&mut self, body: &mut dyn Body) {
        if self.frozen.is_some() || self.slowed {
            body.set_speed(1.0);
        }
        self.slowed = false;
        if let Some(was) = self.frozen.take() {
            body.set_ai(was);
        }
        self.phase = Phase::Over;
    }

    fn step(&mut self, targets: &mut ErTargets, dt: f32) {
        let Some(chr) = crate::targets::find_chr(&self.handle) else {
            self.t += dt;
            self.phase = Phase::Refused;
            return;
        };
        let id = self.id;
        self.run(&mut GameBody { chr, targets, id }, dt);
    }

    /// One step of the ladder on `body`.
    fn run(&mut self, body: &mut dyn Body, dt: f32) {
        self.t += dt;
        if body.hp() <= 0 {
            self.phase = Phase::Over;
            return;
        }
        if self.step == 0 {
            if let Some(why) = body.no_dead().filter(|_| self.down.is_none()) {
                erlog!("stance: target {}: refused: {why}", self.id.0);
                self.phase = Phase::Refused;
                return;
            }
        }
        if let Some((at, shown)) = self.cut {
            let now = body.now();
            if now.id != shown {
                erlog!("stance: target {}: {:.2} s after W_Idle it plays {} ({:.2} of {:.2} s)", self.id.0, self.t - at, now.id, now.time, now.length);
                self.cut = Some((at, now.id));
            }
            if self.t - at > 1.0 {
                self.cut = None;
                self.finish(body);
            }
            return;
        }
        if let Some((at, anim)) = self.down {
            if !self.outro && (self.released || (self.window > 0.0 && self.t - at >= self.window)) {
                if std::mem::take(&mut self.slowed) {
                    body.set_speed(1.0);
                }
                let now = body.now();
                if self.released && self.slam {
                    // the Breaker's slam: the kneel ends for the slam's own reaction (goofy_breaker.rs `slam`)
                    let cut = body.fire("W_Idle");
                    erlog!("stance: target {}: the Breaker's slam: W_Idle asked ({cut}) at {:.2} s of the kneel {anim}", self.id.0, now.time);
                    self.cut = Some((self.t, now.id));
                    return;
                }
                // (COMBAT-FIX) the window ran out with no slam: the game's kneel plays on to its own recovery
                self.outro = true;
                erlog!("stance: target {}: window of {:.2} s over: the kneel {anim} plays on to the game's own recovery ({:.2} of {:.2} s)", self.id.0, self.window, now.time, now.length);
            }
            let now = body.now();
            // (GF-BREAK) the hold is longer than what is left of the kneel: it plays slower, up to the slam
            if let Some(hold) = self.hold.take().filter(|_| !self.outro) {
                let left = (now.length - now.time).max(0.0);
                if now.id == anim && left < hold {
                    body.set_speed((left / hold.max(0.01)).max(SLOWEST));
                    self.slowed = true;
                    erlog!("stance: target {}: the kneel has {left:.2} s left for a hold of {hold:.2} s: played slower", self.id.0);
                }
            }
            if now.id != anim && body.damage_anim().is_none_or(|a| a.id != anim) && self.t - at > 0.2 {
                erlog!("stance: target {}: the kneel {anim} ended by itself after {:.2} s (now {})", self.id.0, self.t - at, now.id);
                self.finish(body);
                return;
            }
            self.phase = Phase::Down { step: self.step, left: if self.window > 0.0 && !self.outro { (self.window - (self.t - at)).max(0.0) } else { (now.length - now.time).max(0.0) } };
            return;
        }
        if self.step == 3 {
            if self.released || self.t >= self.wait + self.window.max(1.5) {
                erlog!("stance: target {}: frozen window over", self.id.0);
                self.finish(body);
            } else {
                self.phase = Phase::Frozen { left: (self.wait + self.window.max(1.5) - self.t).max(0.0) };
            }
            return;
        }
        // (GF-BREAK) released before anything played: the ladder stops here
        if self.released {
            self.finish(body);
            return;
        }
        if let Some(a) = body.damage_anim().filter(|a| is_kneel(a.id)) {
            self.down = Some((self.t, a.id));
            self.phase = Phase::Down { step: self.step, left: if self.window > 0.0 { self.window } else { (a.length - a.time).max(0.0) } };
            erlog!("stance: target {}: DOWN by step {}: animation {} of {:.2} s, {:.2} s after the ask", self.id.0, self.step, a.id, a.length, self.t);
            return;
        }
        if self.step > 0 && self.t < self.wait {
            return;
        }
        self.wait = self.t + STEP_WAIT;
        self.step += 1;
        match self.step {
            1 => match body.ask_break() {
                Ok(row) => erlog!("stance: target {}: step 1: Elden Ring's own stance break asked through its damage (bullet row {row}: poise past its meter, no dmgLevel), it plays {}", self.id.0, body.now().id),
                Err(why) => {
                    erlog!("stance: target {}: step 1: no stance-break bullet ({why}): its W_SABreak next", self.id.0);
                    self.wait = self.t;
                }
            },
            2 => {
                let asked = body.fire("W_SABreak");
                erlog!("stance: target {}: step 2: W_SABreak asked ({asked}), it plays {}", self.id.0, body.now().id);
            }
            _ => {
                self.frozen = Some(body.ai_on());
                body.set_ai(false);
                body.set_speed(0.0);
                self.wait = self.t;
                erlog!("stance: target {}: step 3: no kneel: frozen in {} with its AI off", self.id.0, body.now().id);
            }
        }
    }
}

/// Once a frame.
pub fn frame(targets: &mut ErTargets, dt: f32) {
    // (GF-BREAK) the bodies that waited to land
    let waits = std::mem::take(&mut *WAITS.lock().unwrap_or_else(|e| e.into_inner()));
    for (id, left) in waits {
        let left = left - dt;
        if left <= 0.0 {
            continue;
        }
        if targets.puppets.airborne(id) {
            WAITS.lock().unwrap_or_else(|e| e.into_inner()).push((id, left));
        } else {
            targets.puppets.let_go(id);
            begin(targets, id, left);
        }
    }
    let mut list = BREAKS.lock().unwrap_or_else(|e| e.into_inner());
    for b in list.iter_mut().filter(|b| !matches!(b.phase, Phase::Over | Phase::Refused)) {
        b.step(targets, dt);
    }
    // (FINAL-FIX) the held slams as the step left them (a window that ran out ends its hold), published with BREAKS let go
    let now = held_of(&list);
    drop(list);
    *HELD.lock().unwrap_or_else(|e| e.into_inner()) = now;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A kneeling break of `id` as `begin` + the kneel leave it (the handle is never read here).
    fn kneeling_break(id: TargetId) -> Break {
        Break { t: 0.5, step: 1, down: Some((0.1, 8702)), phase: Phase::Down { step: 1, left: 2.0 }, ..fresh(id, 2.5) }
    }

    /// (COMBAT-FIX) A break as `begin` asks it (the handle is never read here).
    fn fresh(id: TargetId, window: f32) -> Break {
        // SAFETY: FieldInsHandle is two plain integers (selector, block id): all zeros is a valid value.
        let handle: FieldInsHandle = unsafe { std::mem::MaybeUninit::zeroed().assume_init() };
        Break::new(id, handle, window)
    }

    /// (COMBAT-FIX) A MOCKED ELDEN RING CHARACTER: what `chranim` / `puppet::damage_anim` would read, what the steps asked of it,
    /// and how the game answers: `breaks_by_damage` (its stance meter broken by the bullet: its own kneel `kneel` plays),
    /// `breaks_by_event` (`W_SABreak` plays it). Its clock runs with [`Mock::tick`]; the kneel ends by itself at its length.
    struct Mock {
        hp: i32,
        now: AnimNow,
        damage: Option<AnimNow>,
        kneel: AnimNow,
        breaks_by_damage: bool,
        bullet: Result<u32, String>,
        breaks_by_event: bool,
        fired: Vec<String>,
        asks: u32,
        speed: f32,
        ai: bool,
    }

    impl Mock {
        /// A soldier whose own break is `kneel` (c4311: 8702, 2.50 s, MEASURED), idling.
        fn soldier(kneel: i32, length: f32) -> Mock {
            Mock { hp: 219, now: AnimNow { id: 3_000_000, time: 0.0, length: 2.0 }, damage: None, kneel: AnimNow { id: kneel, time: 0.0, length }, breaks_by_damage: true, bullet: Ok(13), breaks_by_event: true, fired: Vec::new(), asks: 0, speed: 1.0, ai: true }
        }

        fn kneel_now(&mut self) {
            self.now = self.kneel;
            self.damage = Some(self.kneel);
        }

        /// `dt` of the game's time: the clip runs at its speed; the kneel over, it stands (idle 20).
        fn tick(&mut self, dt: f32) {
            self.now.time += dt * self.speed;
            if let Some(d) = self.damage.as_mut() {
                d.time = self.now.time;
            }
            if self.damage.is_some() && self.now.time >= self.now.length {
                self.damage = None;
                self.now = AnimNow { id: 20, time: 0.0, length: 3.0 };
            }
        }
    }

    impl Body for Mock {
        fn hp(&self) -> i32 {
            self.hp
        }
        fn no_dead(&self) -> Option<&'static str> {
            None
        }
        fn now(&self) -> AnimNow {
            self.now
        }
        fn damage_anim(&self) -> Option<AnimNow> {
            self.damage
        }
        fn fire(&mut self, event: &str) -> bool {
            self.fired.push(event.to_string());
            match event {
                "W_SABreak" if self.breaks_by_event => self.kneel_now(),
                "W_Idle" => {
                    self.damage = None;
                    self.now = AnimNow { id: 20, time: 0.0, length: 3.0 };
                }
                _ => {}
            }
            true
        }
        fn set_speed(&mut self, speed: f32) {
            self.speed = speed;
        }
        fn ai_on(&self) -> bool {
            self.ai
        }
        fn set_ai(&mut self, on: bool) {
            self.ai = on;
        }
        fn ask_break(&mut self) -> Result<u32, String> {
            self.asks += 1;
            if self.bullet.is_ok() && self.breaks_by_damage {
                self.kneel_now();
            }
            self.bullet.clone()
        }
    }

    const DT: f32 = 1.0 / 60.0;

    /// Steps `b` on `m` for `seconds` (the break first, then the game's frame, as `pre_physics` runs them).
    fn run_for(b: &mut Break, m: &mut Mock, seconds: f32) {
        for _ in 0..(seconds / DT).round() as u32 {
            if matches!(b.phase, Phase::Over | Phase::Refused) {
                return;
            }
            b.run(m, DT);
            m.tick(DT);
        }
    }

    /// (COMBAT-FIX, THE USER 2026-11-06: "enemies who's stagger is broken by Goofy need to play the animation they play when their
    /// stance meter is broken") A BROKEN ENEMY ENTERS ELDEN RING'S OWN BREAK: the first thing asked is the game's stance break through
    /// its damage (no event, no animation of ours named), and the animation it plays is the character's own (any x8700 of its
    /// set: 8702 for the soldier, 2008700 for Margit); the bullet carries poise past any meter and no dmgLevel.
    #[test]
    fn a_goofy_break_plays_elden_rings_own_stance_break() {
        for (kneel, length) in [(8702, 2.5), (2_008_700, 5.0), (8700, 3.37)] {
            let mut m = Mock::soldier(kneel, length);
            let mut b = fresh(TargetId(987_201), 2.5);
            run_for(&mut b, &mut m, 0.1);
            assert_eq!(m.asks, 1, "the game's own break, asked once");
            assert!(m.fired.is_empty(), "no event asked: {:?}", m.fired);
            assert!(matches!(b.phase, Phase::Down { step: 1, .. }), "{:?}", b.phase);
            assert_eq!(b.down.map(|d| d.1), Some(kneel), "the character's own break animation");
            assert!(b.kneels() && is_kneel(kneel));
            assert_eq!(m.speed, 1.0, "played at the game's speed");
        }
        let a = break_attack(0);
        assert_eq!((a.level, a.phys, a.stamina), (0, 1, 0), "no dmgLevel: the reaction is the game's answer to its broken meter");
        assert_eq!(a.poise, crate::zpose::FORCE_POISE, "past any meter (zpose's own check: above 100 x Margit's 80)");
        assert_eq!(break_attack(5).phys, 5, "the route's landing value when set");
    }

    /// (COMBAT-FIX) A BODY THE BULLET CANNOT BREAK still kneels: no bullet route (or 1 HP) -> the game's `W_SABreak` next frame;
    /// a bullet that broke nothing -> `W_SABreak` after the step's wait; nothing at all -> frozen, then given back.
    #[test]
    fn a_body_the_bullet_breaks_not_gets_the_games_event_then_the_freeze() {
        let mut m = Mock::soldier(8702, 2.5);
        m.bullet = Err("no bullet route now".into());
        let mut b = fresh(TargetId(987_202), 2.5);
        run_for(&mut b, &mut m, 0.05);
        assert_eq!(m.fired, vec!["W_SABreak".to_string()]);
        assert!(matches!(b.phase, Phase::Down { step: 2, .. }), "{:?}", b.phase);

        let mut m = Mock::soldier(8702, 2.5);
        m.breaks_by_damage = false;
        let mut b = fresh(TargetId(987_203), 2.5);
        run_for(&mut b, &mut m, 0.2);
        assert!(m.fired.is_empty(), "the bullet's answer is waited for");
        run_for(&mut b, &mut m, 0.2);
        assert_eq!(m.fired, vec!["W_SABreak".to_string()]);
        assert!(matches!(b.phase, Phase::Down { step: 2, .. }), "{:?}", b.phase);

        let mut m = Mock::soldier(8702, 2.5);
        (m.breaks_by_damage, m.breaks_by_event) = (false, false);
        let mut b = fresh(TargetId(987_204), 2.0);
        run_for(&mut b, &mut m, 1.0);
        assert!(matches!(b.phase, Phase::Frozen { .. }) && m.speed == 0.0 && !m.ai, "{:?}", b.phase);
        run_for(&mut b, &mut m, 2.0);
        assert!(b.phase == Phase::Over && m.speed == 1.0 && m.ai, "given back");
    }

    /// (COMBAT-FIX) THE ENEMY RECOVERS ON ELDEN RING'S TIMING: the book's window (2.5 s) runs out on a kneel the game plays longer
    /// (Margit's 5 s): no `W_Idle`, it kneels on (no reaction over it) and gets up when its own clip ends; with a window as long as
    /// the clip (the soldier's 2.5 s) nothing is cut either.
    #[test]
    fn the_enemy_recovers_on_elden_rings_own_timing() {
        let mut m = Mock::soldier(2_008_700, 5.0);
        let mut b = fresh(TargetId(987_205), 2.5);
        run_for(&mut b, &mut m, 2.0);
        b.release_now(); // the book's window ran out (kh-party goofy/hooks.rs `stagger_step`: Release, no Breaker)
        run_for(&mut b, &mut m, 0.5);
        assert!(!m.fired.contains(&"W_Idle".to_string()), "never cut: {:?}", m.fired);
        assert!(b.outro && b.kneels(), "it kneels on: {:?}", b.phase);
        assert!(matches!(b.phase, Phase::Down { left, .. } if (left - 2.5).abs() < 0.1), "the clip's own time left: {:?}", b.phase);
        run_for(&mut b, &mut m, 3.0);
        assert_eq!(b.phase, Phase::Over, "up when its own clip ended");
        assert!((m.now.id, m.speed) == (20, 1.0) && !b.kneels());

        let mut m = Mock::soldier(8702, 2.5);
        let mut b = fresh(TargetId(987_206), 2.5);
        run_for(&mut b, &mut m, 3.0);
        assert!(m.fired.is_empty() && b.phase == Phase::Over, "{:?} {:?}", m.fired, b.phase);
    }

    /// (COMBAT-FIX) THE BREAKER STILL FIRES on the game's break: its hold holds the slam (Protect the slam) and stretches a kneel
    /// shorter than the wind-up; the slam's release cuts the kneel with `W_Idle` so the slam's own reaction plays, and the speed
    /// goes back.
    #[test]
    fn the_breaker_still_fires_on_the_games_break() {
        let mut m = Mock::soldier(8702, 2.5);
        let mut b = fresh(TargetId(987_207), 2.5);
        run_for(&mut b, &mut m, 0.5);
        assert!(matches!(b.phase, Phase::Down { step: 1, .. }));
        assert!(b.hold_for(2.39), "Protect the slam: held");
        run_for(&mut b, &mut m, 0.1);
        assert!(m.speed < 1.0 && b.slowed, "2.0 s left of the kneel for a 2.39 s wind-up: played slower ({})", m.speed);
        assert!(holds(&b) && b.kneels());
        run_for(&mut b, &mut m, 2.0);
        assert!(b.kneels(), "still down for the whole wind-up: {:?}", b.phase);
        b.release_now(); // the slam is about to land
        assert!(!b.kneels() && !holds(&b), "the slam's: no longer held, no longer kneeling");
        run_for(&mut b, &mut m, DT);
        assert_eq!(m.fired, vec!["W_Idle".to_string()], "cut for the slam's own reaction");
        assert_eq!(m.speed, 1.0);
        run_for(&mut b, &mut m, 1.1);
        assert_eq!(b.phase, Phase::Over);
    }

    /// (COMBAT-FIX) Offline (no player, no bullet route): the stance-break bullet is refused with its reason, nothing is fired.
    #[test]
    fn the_stance_break_bullet_needs_the_bullet_route() {
        let mut t = ErTargets::new(crate::coords::Frame::at(glam::Vec3::ZERO), crate::ermap::Params::default());
        t.list.push(crate::targets::Enemy::stub(2, glam::Vec3::new(300.0, 0.0, 0.0), 45.0, 95.0, 2500, crate::ermap::Weight::Medium));
        assert!(t.ask_stance_break(TargetId(2)).is_err());
        let mut low = crate::targets::Enemy::stub(3, glam::Vec3::new(300.0, 0.0, 0.0), 45.0, 95.0, 1, crate::ermap::Weight::Medium);
        low.hp_game = 1;
        t.list.push(low);
        assert!(t.ask_stance_break(TargetId(3)).is_err_and(|why| why.contains("1 HP")));
    }

    /// (FINAL-FIX, FINAL-FIX launch 5: Donald's Ball Lightning killed target 9 in the Breaker's wind-up, no cap line) PROTECT
    /// THE SLAM WITH THE BREAKS LIST LOCKED: once `hold` is asked the slam is held, also while the list is locked (as `frame`
    /// holds it while it steps the breaks); every other party hit then leaves the enemy 1 HP, only the Breaker's own may finish
    /// it; the slam's `release` and a window that ran out end the hold.
    #[test]
    fn protect_the_slam_holds_while_the_breaks_list_is_contended() {
        let id = TargetId(987_101);
        BREAKS.lock().unwrap_or_else(|e| e.into_inner()).push(kneeling_break(id));
        assert!(!slam_held(id), "no Breaker yet");
        hold(id, 2.39);
        let kh_of_er = |hp: i32| hp as f32 * 2.0; // 1 ER HP = 2 KH damage here
        {
            let _stepping = BREAKS.lock().unwrap_or_else(|e| e.into_inner()); // `frame` is stepping the breaks
            assert!(slam_held(id), "held, whatever holds the BREAKS lock (the old try_lock said false here)");
            // Donald's Ball Lightning asking 552 ER (1104 KH) of a soldier with 218 HP: capped inside the 217 above 1 (x0.9: 195 ER)
            let asked = 1104.0;
            let dealt = if slam_held(id) { crate::targets::protect_slam(asked, 0, 218, 218, kh_of_er) } else { asked };
            assert_eq!(dealt, 390.0, "the enemy keeps at least 1 HP until the Breaker lands");
            // an AI's hit, the same; the Breaker's own slam is untouched (its defeat rule decides)
            assert_eq!(crate::targets::protect_slam(asked, kh_core::host::HIT_FLAG_AI, 218, 218, kh_of_er), 390.0);
            assert_eq!(crate::targets::protect_slam(asked, kh_core::host::HIT_FLAG_BREAKER, 218, 218, kh_of_er), asked);
        }
        // `frame`'s publish keeps it while the window runs, and drops it when the window is over
        let mut list = BREAKS.lock().unwrap_or_else(|e| e.into_inner());
        assert!(held_of(&list).contains(&id));
        // the step that slows the kneel takes `hold` (Break::step): the slam stays held (the old rule read `hold` and lost it here)
        for b in list.iter_mut().filter(|b| b.id == id) {
            b.hold = None;
        }
        assert!(held_of(&list).contains(&id), "held for the whole wind-up, after the slow-down took its ask");
        let mut over = kneeling_break(id);
        over.slam = true;
        over.phase = Phase::Over;
        assert!(!held_of(std::slice::from_ref(&over)).contains(&id), "a window that ran out holds no slam");
        drop(list);
        // the slam's own release: from then on the hit is the slam's
        release(id);
        assert!(!slam_held(id), "released by the slam");
        list = BREAKS.lock().unwrap_or_else(|e| e.into_inner());
        list.retain(|b| b.id != id);
    }

    /// (FINAL-FIX, FINAL-FIX launches 5 / 6: two Ball Lightning bullets in one wind-up, each asked to leave 1 of the game's 218 HP,
    /// killed it together) THE BULLETS IN FLIGHT SEE EACH OTHER: two (and more) asked before the game takes any stay together
    /// inside the room above 1, also when the game takes a little more than estimated (x1.05); batch after batch the enemy keeps
    /// at least 1 HP, and only the Breaker's own hit may finish it.
    #[test]
    fn protect_the_slam_counts_the_hits_still_in_flight() {
        let kh_of_er = |hp: i32| hp as f32 * 2.0; // 1 ER HP = 2 KH damage here
        let (mut game, mut ours) = (218, 218);
        for _batch in 0..10 {
            let mut flight = 0;
            for _bullet in 0..3 {
                let asked = crate::targets::protect_slam_room(1104.0, 0, crate::targets::protect_room(game, ours), kh_of_er);
                let er = (asked / 2.0) as i32;
                ours -= er; // our side counts it at the ask
                flight += er;
            }
            assert!(flight <= ((game - 1) as f32 * crate::targets::PROTECT_SHARE) as i32, "the bullets in flight stay inside the room: {flight} of {game}");
            game -= (flight as f32 * 1.05).ceil() as i32; // they land, a little harder than estimated
            assert!(game >= 1, "the game keeps {game} HP");
            ours = game; // the next frame reads the game's number again
        }
        assert!(game <= 12, "the hits close in on 1 from above: {game}");
        assert_eq!(crate::targets::protect_slam_room(1104.0, 0, crate::targets::protect_room(1, 1), kh_of_er), 0.0, "at 1 HP nothing more");
        assert_eq!(crate::targets::protect_slam_room(1104.0, kh_core::host::HIT_FLAG_BREAKER, 0, kh_of_er), 1104.0, "the Breaker's own hit finishes it");
    }
}
