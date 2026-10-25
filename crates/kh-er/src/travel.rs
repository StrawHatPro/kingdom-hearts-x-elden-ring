//! (IMPL_ER23A, user decision 2026-10-13: "Make fast travel impossible unless you've accessed the map via the save point")
//! FAST TRAVEL ONLY FROM A SAVE POINT. In Kingdom Hearts you travel from save points; here the save-point menu's "Select
//! an Area" opens Elden Ring's map. The map opened any other way (View in the field, the game's own menu, the grace
//! menu) still shows everything, takes markers, zooms: only its travel is refused, BY THE GAME ITSELF.
//!
//! HOW (NOT RUN IN THE GAME: built from the SDK's layout by reading): `FieldArea::enable_fast_travel_event_flag`
//! (SDK comment: "Flag to check if fast travel should be enabled"; FieldArea + 0xA0) names the event flag the game asks
//! before it lets the player travel from the map: where that flag is off the map says its own "cannot travel" (as in the
//! areas the game itself closes). While travel is not allowed the field is pointed at the game's constant OFF flag
//! (6000; its twin 6001 is the constant ON flag: both are READ here first and the rule stays off, with a log line, when
//! they are not off / on in this game), and put back to the game's own value while it is allowed or when Sora stops.
//! The game's own prohibitions (combat, dungeons, the areas it closes) stay on top: while travel is allowed the field
//! holds the game's own flag.
//!
//! THE LATCH (`Latch`, pure): set when "Select an Area" asks the game for its map; kept while the map comes up (4 s to
//! do so) and while it is up; gone 0.5 s after the map closed (a quick re-open through View is then already without
//! it), at a load, a warp, a death. `travel.savepoint_only = 0`: travel as the game has it.

use std::sync::atomic::{AtomicBool, Ordering};

use eldenring::cs::{CSEventFlagMan, FieldArea};
use fromsoftware_shared::FromStatic;

use crate::erlog;

/// The game's constant flags (EMEVD's "always OFF" / "always ON").
const FLAG_OFF: u32 = 6000;
const FLAG_ON: u32 = 6001;

/// "Select an Area" asked for the map (savepoint.rs), not yet taken by the latch.
static ASKED: AtomicBool = AtomicBool::new(false);

/// (IMPL_ER38) Presses of View the save point's map flow has made (mapwatch.rs logs each request's outcome).
static PRESSES: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// savepoint.rs: the save-point menu's "Select an Area" asks the game for its map now (called with each press).
pub fn from_save_point() {
    ASKED.store(true, Ordering::Relaxed);
    PRESSES.fetch_add(1, Ordering::Relaxed);
}

/// (IMPL_ER38) How many presses the save point's map flow has made so far.
pub fn save_point_presses() -> u32 {
    PRESSES.load(Ordering::Relaxed)
}

/// The latch: is travel allowed this frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Latch {
    /// Seconds since it was set (`None`: not set); the map was seen up since; seconds the map has been closed again.
    set_for: Option<f32>,
    map_seen: bool,
    closed_for: f32,
}

impl Latch {
    /// The map must come up this soon after the save point asked for it, s.
    pub const OPEN_WITHIN: f32 = 4.0;
    /// The latch outlives the map's closing by this much, s.
    pub const CLOSED_FOR: f32 = 0.5;

    /// One frame. `asked`: "Select an Area" asked for the map in this frame; `game_menu`: a menu of Elden Ring's has
    /// the input (the map is one); `reset`: a load, a warp, a death. Returns: travel is allowed.
    pub fn frame(&mut self, asked: bool, game_menu: bool, reset: bool, dt: f32) -> bool {
        if reset {
            *self = Latch::default();
        }
        if asked {
            *self = Latch { set_for: Some(0.0), map_seen: false, closed_for: 0.0 };
        }
        let Some(age) = self.set_for.as_mut() else { return false };
        *age += dt;
        if game_menu {
            self.map_seen = true;
            self.closed_for = 0.0;
        } else if self.map_seen {
            self.closed_for += dt;
            if self.closed_for > Self::CLOSED_FOR {
                *self = Latch::default();
                return false;
            }
        } else if *age > Self::OPEN_WITHIN {
            *self = Latch::default();
            return false;
        }
        true
    }
}

/// The game side: what the field held before us, what we wrote.
#[derive(Default)]
pub struct Travel {
    latch: Latch,
    own: Option<i32>,
    blocked: bool,
    allowed_told: Option<bool>,
    unusable: bool,
}

fn flag(id: u32) -> Option<bool> {
    // SAFETY: game thread; the singleton lives as long as the game; the flag blocks are the game's own tables.
    let man = unsafe { CSEventFlagMan::instance() }.ok()?;
    (!man.virtual_memory_flag.flag_blocks.is_null()).then(|| man.virtual_memory_flag.get_flag(id))
}

impl Travel {
    /// Once a frame while Sora runs. `on`: the key `travel.savepoint_only` (default on).
    pub fn frame(&mut self, on: bool, game_menu: bool, reset: bool, dt: f32) {
        let allowed = self.latch.frame(ASKED.swap(false, Ordering::Relaxed), game_menu, reset, dt);
        let block = on && !allowed && !self.unusable;
        // SAFETY: game thread; the pointer is the game's own static.
        let Ok(area) = (unsafe { FieldArea::instance_mut() }) else { return };
        let now = area.enable_fast_travel_event_flag;
        if block {
            if now != FLAG_OFF as i32 {
                // (first time, or the game wrote its own value again: an area change)
                if flag(FLAG_OFF) != Some(false) || flag(FLAG_ON) != Some(true) {
                    self.unusable = true;
                    erlog!("travel: the game's constant flags read {:?} / {:?} (wanted off / on): FAST TRAVEL IS NOT RESTRICTED (the rule cannot be made here)", flag(FLAG_OFF), flag(FLAG_ON));
                    return;
                }
                if self.own != Some(now) || !self.blocked {
                    erlog!("travel: FAST TRAVEL ONLY FROM A SAVE POINT: the game's fast-travel flag of this area ({now}) is replaced by its constant OFF flag ({FLAG_OFF}) until the map is opened through a save point's \"Select an Area\"");
                }
                self.own = Some(now);
                area.enable_fast_travel_event_flag = FLAG_OFF as i32;
                self.blocked = true;
            }
        } else {
            self.give_back(area);
        }
        if on && self.allowed_told != Some(allowed) {
            if self.allowed_told.is_some() || allowed {
                erlog!("travel: {}", if allowed { "the map was asked for at a SAVE POINT: fast travel is the game's own again while this map is up" } else { "the save point's map is closed: fast travel is refused again" });
            }
            self.allowed_told = Some(allowed);
        }
    }

    fn give_back(&mut self, area: &mut FieldArea) {
        if std::mem::take(&mut self.blocked) {
            if let Some(own) = self.own {
                if area.enable_fast_travel_event_flag == FLAG_OFF as i32 {
                    area.enable_fast_travel_event_flag = own;
                }
            }
        }
    }

    /// Sora stops: the game's own value is put back.
    pub fn stop(&mut self) {
        // SAFETY: game thread.
        if let Ok(area) = unsafe { FieldArea::instance_mut() } {
            self.give_back(area);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 1.0 / 60.0;

    #[test]
    fn travel_is_allowed_only_for_the_map_a_save_point_opened() {
        let mut latch = Latch::default();
        // the map opened with View in the field: never allowed
        assert!((0..300).all(|k| !latch.frame(false, k > 30, false, DT)));
        // "Select an Area": allowed while the map comes up (our menu closes, the press is made) and while it is up
        assert!(latch.frame(true, false, false, DT));
        assert!((0..60).all(|_| latch.frame(false, false, false, DT)), "the second before the map is up");
        assert!((0..600).all(|_| latch.frame(false, true, false, DT)), "the map is up");
        // closed: half a second later the latch is gone, and a map opened again by View has none
        let mut frames = 0;
        while latch.frame(false, false, false, DT) {
            frames += 1;
        }
        assert!((frames as f32 * DT - Latch::CLOSED_FOR).abs() < 0.05, "{frames}");
        assert!((0..300).all(|_| !latch.frame(false, true, false, DT)));
    }

    #[test]
    fn a_map_that_never_opens_and_a_load_drop_the_latch() {
        let mut latch = Latch::default();
        latch.frame(true, false, false, DT);
        let frames = (0..1000).take_while(|_| latch.frame(false, false, false, DT)).count();
        assert!((frames as f32 * DT - Latch::OPEN_WITHIN).abs() < 0.05, "{frames}");
        // a warp / load while the map is up (the travel itself): gone at once
        latch.frame(true, false, false, DT);
        assert!(latch.frame(false, true, false, DT));
        assert!(!latch.frame(false, true, true, DT));
    }
}
