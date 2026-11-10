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
//!
//! (MELINA-SUMMON, the user 2026-11-08: "There is definitely no option to talk to Melina after defeating Margit") WHAT THE
//! FIELD ALSO DRIVES (eldenring.exe 2.7.1.0, read in the copy): a per-character update (0x140660d86) puts SpEffect 4270
//! (stateInfo 278) on the player while 0x140621920(FieldArea) reads 0, i.e. while this flag is off (an id of 0 / -1 or no
//! flag manager reads 1), and takes it off when it reads 1. So pointing the field at 6000 kept 4270 on the body all
//! session, and the talk scripts read 4270 as "no travel here": the grace's x57 / x60 and Melina's x72 offer the
//! Roundtable Hold (she warps him there) only without it. Two answers, neither touches the effect itself:
//! - `closed_by_the_game`: what 4270 would say without this rule (the area's own flag, the exe's rule), for the talk
//!   gate while the rule holds the field;
//! - `hold_for_talk`: while Melina's summoned talk runs at a save point the field holds the area's own flag (the save
//!   point is where travel is allowed anyway), so the game itself takes 4270 off before her script tests it.
//!
//! (MELINA-4270b, the user 2026-11-10: "I still can't talk to Melina after beating margit") THE GATE STILL READ THE BODY.
//! The user's log: the open world's own fast-travel flag is 0 ("the game's fast-travel flag of this area (0) is replaced
//! by its constant OFF flag (6000)"), and the old record used 0 for "the field is the game's own": so
//! `closed_by_the_game` answered None in the open world, the gate fell back to the body's 4270 (the lock's), and "Talk
//! to Melina" was never offered ("she would have something to say, but the player carries [4270]"). Now the area's own
//! flag is recorded with "known" apart from its value (`own_flag`), every frame, held or not, and the hold covers every
//! talk of the grace's and Melina's scripts at a save point (her summon, the game's rest at the grace, our rest flow).
//! THE GAME'S PATH, read in eldenring.exe 2.7.2.0 (the user's exe, sha256 f9118fea..; Ghidra, build/sora_tmp/melina_4270/STATE.md):
//! - FUN_140621dd0(FieldArea [0x143d70268]): id = +0xA0; 0 / -1 (or no flag manager [0x143d6f4d8]) -> 1 (open); else the
//!   flag's value. = `travel_closed` negated.
//! - PlayerIns vtable 0x142a81cb0 slot 25 = FUN_140661030 (the player's update): FUN_140621dd0 == 0 -> SpEffect 0x10ae
//!   (4270) put on (FUN_1403e8dc0) if missing, else taken off (0x1403ee2e0) if there. The only two code sites of 4270 in
//!   the exe besides the map's travel refusal FUN_1407a1820 (player [0x143d6d018]+0x1e508, effects 4272 4288 4271 4286
//!   4270 4282: each a "cannot travel" text).
//! - CSEzStateTalkEnv::invoke 0x140ea3b90 `case 0x3d` (query 61): the main player's (+0x1e508) special effects
//!   (+0x178) hold arg 1 (0x1404fa7f0, the same test). Of every talk script of the game (365 in 36 archives) only the
//!   grace's t000001000 (x57's entry 10009656, x60) and Melina's t000003000 (x72) ask query 61 for 4270; no event script
//!   (470) and not c0000.hks reads it. So the lock's 4270 hides nothing but Melina's Roundtable talks and the map's travel.

use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};

use eldenring::cs::{CSEventFlagMan, FieldArea};
use fromsoftware_shared::FromStatic;

use crate::erlog;

/// The game's constant flags (EMEVD's "always OFF" / "always ON").
const FLAG_OFF: u32 = 6000;
const FLAG_ON: u32 = 6001;

/// "Select an Area" asked for the map (savepoint.rs), not yet taken by the latch.
static ASKED: AtomicBool = AtomicBool::new(false);

/// (MELINA-SUMMON) savepoint.rs: a talk of the grace's / Melina's scripts runs at a save point; the area's own fast-travel
/// flag stands meanwhile.
static TALK_HOLD: AtomicBool = AtomicBool::new(false);
/// (MELINA-4270b) The area's own fast-travel flag (the field's value without this rule), as `Travel::frame` saw it last;
/// `UNKNOWN`: not seen (Sora stopped, no FieldArea).
static OWN: AtomicI64 = AtomicI64::new(UNKNOWN);
const UNKNOWN: i64 = i64::MIN;

/// (MELINA-SUMMON) savepoint.rs, every frame: a talk of the grace's or Melina's scripts is under way at a save point (her
/// summon, the game's rest at the grace) (true) or not.
pub fn hold_for_talk(on: bool) {
    TALK_HOLD.store(on, Ordering::Relaxed);
}

/// (MELINA-4270b) Is the hold on (savepoint.rs's log)?
pub fn holding_for_talk() -> bool {
    TALK_HOLD.load(Ordering::Relaxed)
}

fn note_own(own: Option<i32>) {
    OWN.store(own.map_or(UNKNOWN, i64::from), Ordering::Relaxed);
}

/// (MELINA-4270b) The area's own fast-travel flag id (what FieldArea + 0xA0 holds without this rule), None: not known.
pub fn own_flag() -> Option<i32> {
    i32::try_from(OWN.load(Ordering::Relaxed)).ok()
}

/// (MELINA-SUMMON) THE EXE'S RULE FOR SPEFFECT 4270 (0x140621920): travel is closed when the fast-travel flag `id` is a
/// real flag (not 0, not -1) and reads off; an unreadable flag reads open.
pub fn travel_closed(id: i32, flag: &dyn Fn(u32) -> Option<bool>) -> bool {
    id != 0 && id != -1 && flag(id as u32) == Some(false)
}

/// (MELINA-SUMMON) Would the game itself close travel here (= would 4270 be on without this rule: the exe's rule on the
/// area's own flag)? (MELINA-4270b) Answered whenever the area's own flag is known, held or not, also for the open
/// world's flag 0 (open); None only without it (then the body's 4270 is the game's).
pub fn closed_by_the_game(flag: &dyn Fn(u32) -> Option<bool>) -> Option<bool> {
    own_flag().map(|own| travel_closed(own, flag))
}

/// (MELINA-4270b) WHAT THE TALK SCRIPTS' QUERY 61 WOULD READ for the player's special effect `id` WITHOUT THIS RULE: for
/// 4270 the game's own answer (`closed_by_the_game`), for every other effect (and without that answer) the body's.
pub fn carries_for_talk(id: i32, game_closed: Option<bool>, body: &dyn Fn(i32) -> bool) -> bool {
    match game_closed {
        Some(closed) if id == crate::talkdrive::NO_TRAVEL => closed,
        _ => body(id),
    }
}

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
        let hold = TALK_HOLD.load(Ordering::Relaxed);
        let allowed = self.latch.frame(ASKED.swap(false, Ordering::Relaxed), game_menu, reset, dt) || hold;
        let block = on && !allowed && !self.unusable;
        // SAFETY: game thread; the pointer is the game's own static.
        let Ok(area) = (unsafe { FieldArea::instance_mut() }) else {
            note_own(None);
            return;
        };
        let own = self.on_field(&mut area.enable_fast_travel_event_flag, block, &|| (flag(FLAG_OFF), flag(FLAG_ON)));
        note_own(Some(own));
        if on && self.allowed_told != Some(allowed) {
            if self.allowed_told.is_some() || allowed {
                erlog!("travel: {}", if allowed && hold { format!("(MELINA-4270b) a talk of the grace's / Melina's scripts runs at a save point (her summon or the game's rest at its grace): the area's own fast-travel flag ({own}) stands meanwhile, so the game takes its 4270 off and her talks (the Roundtable Hold) are as in the game") } else if allowed { "the map was asked for at a SAVE POINT: fast travel is the game's own again while this map is up".to_string() } else { "the save point's map is closed / the talk at the save point is over: fast travel is refused again".to_string() });
            }
            self.allowed_told = Some(allowed);
        }
    }

    /// (MELINA-4270b) THE RULE ON THE FIELD (FieldArea + 0xA0; pure on its value so the tests run it): `block`: travel is
    /// refused this frame; `constants`: the game's constant flags 6000 / 6001 as read. Returns the area's own fast-travel
    /// flag (the field's value without this rule; while held, the one it held before).
    fn on_field(&mut self, field: &mut i32, block: bool, constants: &dyn Fn() -> (Option<bool>, Option<bool>)) -> i32 {
        let now = *field;
        if block {
            if now != FLAG_OFF as i32 {
                // (first time, or the game wrote its own value again: an area change)
                let (off, on) = constants();
                if off != Some(false) || on != Some(true) {
                    self.unusable = true;
                    erlog!("travel: the game's constant flags read {off:?} / {on:?} (wanted off / on): FAST TRAVEL IS NOT RESTRICTED (the rule cannot be made here)");
                    return now;
                }
                if self.own != Some(now) || !self.blocked {
                    erlog!("travel: FAST TRAVEL ONLY FROM A SAVE POINT: the game's fast-travel flag of this area ({now}) is replaced by its constant OFF flag ({FLAG_OFF}) until the map is opened through a save point's \"Select an Area\"");
                }
                self.own = Some(now);
                *field = FLAG_OFF as i32;
                self.blocked = true;
            }
        } else {
            self.give_back(field);
        }
        if self.blocked {
            self.own.unwrap_or(now)
        } else {
            *field
        }
    }

    fn give_back(&mut self, field: &mut i32) {
        if std::mem::take(&mut self.blocked) {
            if let Some(own) = self.own {
                if *field == FLAG_OFF as i32 {
                    *field = own;
                }
            }
        }
    }

    /// Sora stops: the game's own value is put back.
    pub fn stop(&mut self) {
        note_own(None);
        // SAFETY: game thread.
        if let Ok(area) = unsafe { FieldArea::instance_mut() } {
            self.give_back(&mut area.enable_fast_travel_event_flag);
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

    /// (MELINA-SUMMON) the exe's rule for SpEffect 4270, and the answer the talk gate gets while the rule holds the field
    #[test]
    fn effect_4270_is_the_areas_own_travel_flag_off() {
        let flags = |on: &'static [u32]| move |f: u32| Some(on.contains(&f));
        assert!(travel_closed(6000, &flags(&[6001])), "the constant OFF flag: closed (why 4270 stayed on)");
        assert!(!travel_closed(6001, &flags(&[6001])));
        assert!(!travel_closed(0, &flags(&[])) && !travel_closed(-1, &flags(&[])), "no flag: open");
        assert!(!travel_closed(1043, &|_| None), "unreadable: open");
        assert!(travel_closed(1043, &flags(&[])) && !travel_closed(1043, &flags(&[1043])));
    }

    /// The process-wide record (`OWN`) is touched by one test at a time.
    static OWN_TESTS: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// (MELINA-4270b) THE USER'S LOG OF 2026-11-10, replayed: after Margit, at grace 10001958 (m10) and Gatefront
    /// 1042371950 (m60), "Talk to Melina" was refused with "she would have something to say, but the player carries
    /// [4270] ... 4270: duration -1 s, state info 278", while "story.roundtable ... READY (its flags hold)" and "the game's
    /// fast-travel flag of this area (0) is replaced by its constant OFF flag (6000)". FAILS ON THE OLD CODE (DEPLOY7..10:
    /// own flag 0 was its "not held" mark, so the gate read the body's 4270, `talk_for` None).
    #[test]
    fn the_logs_save_after_margit_offers_talk_to_melina_although_the_lock_put_4270_on_him() {
        let _one = OWN_TESTS.lock().unwrap_or_else(|e| e.into_inner());
        // the lock takes the field as in the log: the area's own flag 0 -> 6000
        let mut travel = Travel::default();
        let mut field = 0;
        let own = travel.on_field(&mut field, true, &|| (Some(false), Some(true)));
        assert_eq!((field, own), (FLAG_OFF as i32, 0), "the lock holds the field, the area's own flag is 0");
        note_own(Some(own));
        // the save's flags: story.accord DONE (951, 953 on), story.roundtable READY (4680; 10000851 = Margit's
        // introduction; 10009655 / 104 / 105 off), story.morgott not yet (9104 off)
        let save = |f: u32| Some([951, 953, 4680, 10000851].contains(&f));
        // the body: 4270, put on by the player's update (FUN_140661030) because 6000 reads off
        let body = |e: i32| e == crate::talkdrive::NO_TRAVEL;
        assert!(travel_closed(field, &save), "the field as held: closed, hence the body's 4270");
        assert_eq!(closed_by_the_game(&save), Some(false), "the open world's own flag 0: open, as FUN_140621dd0 answers");
        for area in [10, 60] {
            let has = |id: i32| carries_for_talk(id, closed_by_the_game(&save), &body);
            assert!(!has(4270), "4270 is the lock's, not the game's");
            assert_eq!(crate::talkdrive::talk_for(&save, &has, area), Some(crate::talkdrive::Talk::Own), "area {area}: her own talk (the grace's x60: the Roundtable invitation)");
            // the body itself would refuse it (what the old gate did)
            assert_eq!(crate::talkdrive::talk_for(&save, &body, area), None, "area {area}");
        }
        // the same with a grace outside Limgrave found instead of Margit's scene (3062), and after Morgott (9104)
        let outside = |f: u32| Some([951, 953, 4680, 3062].contains(&f));
        assert_eq!(crate::talkdrive::talk_for(&outside, &|id| carries_for_talk(id, closed_by_the_game(&outside), &body), 60), Some(crate::talkdrive::Talk::Own));
        let morgott = |f: u32| Some([951, 953, 4680, 10000851, 10009655, 104, 105, 9104].contains(&f));
        assert_eq!(crate::talkdrive::talk_for(&morgott, &|id| carries_for_talk(id, closed_by_the_game(&morgott), &body), 60), Some(crate::talkdrive::Talk::Own));
        // an area whose own flag is real and off: the game closes travel itself, 4270 counts as in the game
        note_own(Some(1043));
        assert_eq!(closed_by_the_game(&save), Some(true));
        assert!(carries_for_talk(4270, closed_by_the_game(&save), &|_| false), "the game's own closure is kept");
        assert_eq!(crate::talkdrive::talk_for(&save, &|id| carries_for_talk(id, closed_by_the_game(&save), &body), 60), None);
        // the other effects are always the body's
        assert!(carries_for_talk(4271, Some(false), &|e| e == 4271));
        // not known (Sora stopped): the body's
        note_own(None);
        assert_eq!(closed_by_the_game(&save), None);
        assert!(carries_for_talk(4270, closed_by_the_game(&save), &body));
    }

    /// (MELINA-4270b) the hold (her talk, the game's rest at the grace) gives the field back at once, so the player's update
    /// reads the area's own flag (open) and takes 4270 off; after it the lock takes the field again.
    #[test]
    fn the_hold_gives_the_area_its_own_flag_and_the_lock_comes_back() {
        let mut travel = Travel::default();
        let mut field = 0;
        let consts = || (Some(false), Some(true));
        assert_eq!(travel.on_field(&mut field, true, &consts), 0);
        assert_eq!(field, 6000);
        assert_eq!(travel.on_field(&mut field, false, &consts), 0, "held: the area's own");
        assert_eq!(field, 0);
        assert!(!travel_closed(field, &|_| Some(false)), "the exe's rule: open, 4270 comes off");
        assert_eq!(travel.on_field(&mut field, true, &consts), 0);
        assert_eq!(field, 6000, "the lock again after the talk");
        // an area change while locked: the game's new value is taken as the own one
        field = 1043;
        assert_eq!(travel.on_field(&mut field, true, &consts), 1043);
        assert_eq!(field, 6000);
        // constants not as expected: never written
        let mut other = Travel::default();
        let mut f = 77;
        assert_eq!(other.on_field(&mut f, true, &|| (Some(true), Some(true))), 77);
        assert_eq!(f, 77);
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
