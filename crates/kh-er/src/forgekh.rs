//! (P7 SMITHS, THE USER 2026-11-03: KH3's Keyblade Forge "accessible from any instance Elden Ring's blacksmithing menu would
//! appear"; "Remove all options from blacksmiths' dialogue options related to ashes of war and other stuff like that, their
//! only mechanical use to the player is upgrades") THE KEYBLADE FORGE AT EVERY SMITH, THE GAME HALF (the boc.rs /
//! spirittune.rs pattern). Plan: documentation/KH3_WEAPONS_PLAN.md 1.6. UNTESTED IN THE GAME.
//!
//! - THE TALK: talkkh/smith.rs (held by the hooks' ShopTalk) swallows command 24 (OpenEnhanceShop) of any character's talk
//!   script and leaves a request, trims the smiths' other options and keeps their script waiting (query 59) while the Forge
//!   stands in. [`talk_frame`] keeps its key and its sheet rows there, opens KH3's Workshop straight on its Keyblade Forge
//!   for the request ([`ForgeScreen`]: P6b's screen, kh-host hud_workshop.rs) and, when the Forge has closed (B / back),
//!   lets the script read the menu as closed: the smith's talk list comes back. Not up within 3 s, never shown, or a
//!   cutscene: given up the same way (his talk goes on; Elden Ring's reinforcement menu still never opens).
//! - Roderika's spirit tuning is not this (spirittune.rs, already a KH3 screen).
//!
//! KEYS: `forge.kh` (1; 0 = the older SHOP rule: `npc.smith_talk` decides, Elden Ring's menu or a refused prompt). Logs:
//! `forge:` lines (the trims, the request, the Forge opening / closing, why a request was given up).

use std::sync::Mutex;

use crate::control::Control;
use crate::erlog;
use crate::talkkh::smith;

/// KH3's Keyblade Forge as the smiths need it: P6b's Workshop screen (kh-host hud_workshop.rs, `open(Entry::Forge)` with
/// a back / closed result) answers it through [`with_screen`].
pub trait ForgeScreen {
    /// This build has the screen.
    fn available(&self) -> bool {
        true
    }
    /// Ask for the Workshop opened straight on its Keyblade Forge. `true`: taken (it may come up a few frames later).
    fn open_forge(&mut self) -> bool;
    /// (closed: left with B / back or gone; shown: on screen now).
    fn forge_state(&mut self) -> (bool, bool);
    /// Take it down (a cutscene began) or drop a request that never came up.
    fn close_forge(&mut self);
}

/// THE SCREEN THE REQUESTS OPEN: (P6b, INT) KH3's Workshop on its Keyblade Forge.
fn with_screen<R>(f: impl FnOnce(&mut dyn ForgeScreen) -> R) -> Option<R> {
    Some(f(&mut Workshop))
}

/// (P6b) KH3's Workshop on its Keyblade Forge (kh-host hud_workshop.rs).
struct Workshop;

impl ForgeScreen for Workshop {
    fn open_forge(&mut self) -> bool {
        kh_host::hud_workshop::open(kh_host::hud_workshop::Entry::Forge, "", &|f| crate::bossreward::game_flag(f) == Some(true))
    }
    fn forge_state(&mut self) -> (bool, bool) {
        // (INT) shown = UP on the HUD (`is_up`): a request the HUD never takes is given up after 3 s (forge_step) and closed
        let up = kh_host::hud_workshop::is_up();
        (matches!(kh_host::hud_workshop::take_result(), Some(kh_host::hud_workshop::Outcome::Closed(_))) || !kh_host::hud_workshop::is_open(), up)
    }
    fn close_forge(&mut self) {
        kh_host::hud_workshop::close();
    }
}

/// The talk hooks must run for the Forge (even with `talk.kh = 0` and the shop off).
pub fn wants_talk_hooks(control: &Control) -> bool {
    crate::talkkh::hooks::COMPLETE.load(std::sync::atomic::Ordering::Relaxed) && control.flag("forge.kh") != Some(false)
}

/// What [`forge_step`] says to do this frame.
#[derive(Clone, Copy, Debug, PartialEq)]
enum ForgeStep {
    Wait,
    /// The screen took the request.
    Opened,
    /// The request is done (why): the script reads the menu as closed. `close`: take the screen down first.
    Done(&'static str, bool),
}

const CLOSED: &str = "closed";
const NOT_SHOWN: &str = "the Forge did not come up";
const NOT_BUILT: &str = "the Keyblade Forge screen is not in this build yet (P6b)";
/// Seconds a request may wait for the Forge.
const WAIT: f32 = 3.0;

/// THE REQUEST'S STEPS (pure; [`talk_frame`] gives it the screen): `opened` = the request's state (None: no request);
/// `available` = the screen exists; `open` asks for it; `state` = (closed, shown).
fn forge_step(opened: Option<bool>, cutscene: bool, available: bool, open: impl FnOnce() -> bool, state: impl FnOnce() -> (bool, bool), wait: &mut f32, dt: f32) -> ForgeStep {
    let step = match opened {
        None => ForgeStep::Wait,
        Some(false) if !available => ForgeStep::Done(NOT_BUILT, false),
        Some(false) if cutscene => ForgeStep::Done("a cutscene began", false),
        Some(false) if open() => ForgeStep::Opened,
        Some(false) => {
            *wait += dt;
            if *wait > WAIT { ForgeStep::Done("the Forge could not open (another menu is up, or no HUD)", false) } else { ForgeStep::Wait }
        }
        Some(true) if cutscene => ForgeStep::Done("a cutscene began", true),
        Some(true) => match state() {
            (true, _) => ForgeStep::Done(CLOSED, false),
            (false, true) => {
                *wait = 0.0;
                ForgeStep::Wait
            }
            (false, false) => {
                *wait += dt;
                if *wait > WAIT { ForgeStep::Done(NOT_SHOWN, true) } else { ForgeStep::Wait }
            }
        },
    };
    if matches!(step, ForgeStep::Opened | ForgeStep::Done(..)) || opened.is_none() {
        *wait = 0.0;
    }
    step
}

/// ONCE A FRAME (sora.rs, after the talk adapter, the KH shop, Boc and Roderika): see the file header.
pub fn talk_frame(control: &Control, cutscene: bool, dt: f32) {
    use crate::talkkh::hooks::with_shared;
    static WAITED: Mutex<f32> = Mutex::new(0.0);
    static TOLD: Mutex<bool> = Mutex::new(false);
    let on = control.flag("forge.kh") != Some(false);
    let rules = smith::rules();
    {
        let mut told = TOLD.lock().unwrap_or_else(|e| e.into_inner());
        if !*told {
            *told = true;
            erlog!("forge: sheet sora-er-shop: command {} -> the Keyblade Forge in any character's talk (forge.kh {}), menu types {:?}, smiths {:?}, trimmed texts {:?}, swallowed in their talk {:?}{}", rules.open_enhance, if on { "on" } else { "OFF" }, rules.menu_types, rules.talk_ids, rules.trim, rules.swallow, if rules.problems.is_empty() { String::new() } else { format!(" | PROBLEM ROW(S): {}", rules.problems.join("; ")) });
        }
    }
    let Some((request, notes)) = with_shared(|s| {
        let t = &mut s.shop.smith;
        t.on = on;
        if t.rules != *rules {
            t.rules = rules.clone();
        }
        (t.request.clone(), std::mem::take(&mut t.notes))
    }) else {
        return;
    };
    for n in notes {
        erlog!("forge: {n}");
    }
    let mut waited = WAITED.lock().unwrap_or_else(|e| e.into_inner());
    let available = with_screen(|s| s.available()).unwrap_or(false);
    let step = forge_step(request.as_ref().map(|r| r.opened), cutscene, available, || with_screen(|s| s.open_forge()).unwrap_or(false), || with_screen(|s| s.forge_state()).unwrap_or((true, false)), &mut waited, dt);
    match step {
        ForgeStep::Wait => {}
        ForgeStep::Opened => {
            if let Some(r) = request {
                with_shared(|s| {
                    if let Some(q) = s.shop.smith.request.as_mut().filter(|q| q.serial == r.serial) {
                        q.opened = true;
                    }
                });
                let name = crate::talkkh::game::npc_of(r.ins).map(|(_, name)| name).unwrap_or_default();
                erlog!("forge: KH3's Keyblade Forge opens for talk {} ({}) in place of Elden Ring's reinforcement menu (command 24({}))", r.talk_id, if name.is_empty() { "no name" } else { &name }, r.kind);
            }
        }
        ForgeStep::Done(why, close) => {
            if close {
                with_screen(|s| s.close_forge());
            }
            with_shared(|s| s.shop.smith.done(why));
            if why != CLOSED {
                erlog!("forge: the smith's reinforcement menu is given up: {why} (his talk goes on; Elden Ring's menu does not open)");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// (THE USER 2026-11-03) A SMITH'S REQUEST: the Forge is asked for at once; once up it lasts while it is shown and is
    /// done when it closes (B / back: his talk list comes back); one that never opens or never comes up, a cutscene, or no
    /// Forge in the build gives the request up (his talk goes on).
    #[test]
    fn a_smiths_request_opens_the_forge_and_back_returns_to_his_talk() {
        let mut wait = 0.0;
        let never = || -> bool { panic!("not asked") };
        let no_state = || -> (bool, bool) { panic!("not asked") };
        assert_eq!(forge_step(None, false, true, never, no_state, &mut wait, 0.1), ForgeStep::Wait, "no request");
        assert_eq!(forge_step(Some(false), false, true, || true, no_state, &mut wait, 0.1), ForgeStep::Opened);
        assert_eq!(forge_step(Some(true), false, true, never, || (false, false), &mut wait, 0.5), ForgeStep::Wait, "asked, not on screen yet");
        assert_eq!(forge_step(Some(true), false, true, never, || (false, true), &mut wait, 0.5), ForgeStep::Wait);
        assert_eq!(wait, 0.0, "shown: no waiting counted");
        for _ in 0..600 {
            assert_eq!(forge_step(Some(true), false, true, never, || (false, true), &mut wait, 0.1), ForgeStep::Wait, "up as long as the player likes");
        }
        assert_eq!(forge_step(Some(true), false, true, never, || (true, false), &mut wait, 0.1), ForgeStep::Done(CLOSED, false), "B: back to the smith's list");
        // the HUD refuses (another menu up): given up after 3 s
        let mut wait = 0.0;
        for _ in 0..29 {
            assert_eq!(forge_step(Some(false), false, true, || false, no_state, &mut wait, 0.1), ForgeStep::Wait);
        }
        assert!(matches!(forge_step(Some(false), false, true, || false, no_state, &mut wait, 0.2), ForgeStep::Done(why, false) if why != CLOSED));
        // taken but never shown: given up and taken down
        let mut wait = 0.0;
        for _ in 0..29 {
            forge_step(Some(true), false, true, never, || (false, false), &mut wait, 0.1);
        }
        assert_eq!(forge_step(Some(true), false, true, never, || (false, false), &mut wait, 0.2), ForgeStep::Done(NOT_SHOWN, true));
        // a cutscene before / while it is up; no Forge in this build (the stub)
        assert_eq!(forge_step(Some(false), true, true, never, no_state, &mut 0.0, 0.1), ForgeStep::Done("a cutscene began", false));
        assert_eq!(forge_step(Some(true), true, true, never, no_state, &mut 0.0, 0.1), ForgeStep::Done("a cutscene began", true));
        assert_eq!(forge_step(Some(false), false, false, never, no_state, &mut 0.0, 0.1), ForgeStep::Done(NOT_BUILT, false));
        assert_eq!(with_screen(|s| s.available()), Some(true), "(INT) P6b's Workshop is the screen");
    }
}
