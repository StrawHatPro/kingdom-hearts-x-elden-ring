//! (IMPL_ER26, user decisions 2026-10-13: "remove attuning to sites of grace and sitting at sites of grace, when you run
//! into a save point it should unlock on the map as fast travel", "Include sites of grace change time of day as an option
//! in the save point menu", "Include 'Respawn Enemies' as an option") THE PURE RULES of the save points at the sites of
//! grace: no game access here (savepoint.rs applies them), unit-tested below.
//! State note: build/sora_tmp/er/IMPL_ER26_STATE.md.

/// AUTO-UNLOCK of one save point: what to do this frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnlockStep {
    Nothing,
    /// Write the grace's discovery flag now (and read it back).
    Write,
    /// The flag was written `Unlock::TRIES` times and never read back as set: nothing more is tried (said once).
    GaveUp,
}

/// THE UNLOCK RULE: a save point the game presents (its grace's map object plays its flame) whose grace is not
/// discovered is unlocked the moment Sora's capsule is in its Save-command capsule (the reactor, radius 110 cm: the
/// larger of the save point's two capsules, so that running past it is enough). No animation, no prompt, no press.
/// A hidden grace (a boss's before the boss is defeated, one an event has not enabled) is never unlocked, and one
/// whose state is not known yet neither.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Unlock {
    tries: u8,
    wait: f32,
    gave_up: bool,
}

impl Unlock {
    /// Writes that may be made for one save point while it stands (each is read back at once).
    pub const TRIES: u8 = 3;
    /// Seconds between two writes.
    pub const RETRY: f32 = 1.0;

    /// One frame. `on`: the key `savepoint.autounlock` (and not `savepoint.sit`); `presented`: the game shows this
    /// grace (flame seen: NOT "taken as present"); `discovered`: the flag as read (`None` = not readable);
    /// `in_reactor`: his capsule overlaps the Save-command capsule.
    pub fn step(&mut self, on: bool, presented: bool, discovered: Option<bool>, in_reactor: bool, dt: f32) -> UnlockStep {
        self.wait = (self.wait - dt).max(0.0);
        if !on || !presented || discovered != Some(false) || !in_reactor || self.gave_up || self.wait > 0.0 {
            return UnlockStep::Nothing;
        }
        if self.tries >= Self::TRIES {
            self.gave_up = true;
            return UnlockStep::GaveUp;
        }
        self.tries += 1;
        self.wait = Self::RETRY;
        UnlockStep::Write
    }

    pub fn tries(&self) -> u8 {
        self.tries
    }
}

/// WHOSE IS Y near a save point (the press-ownership rule: the owner at the down-edge keeps the whole press).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct YRule {
    /// Y opens KH3's save-point menu.
    pub offered: bool,
    /// Y is the save point's (hidden from Elden Ring and from Sora), whether or not the menu opens: the menu, or a
    /// prompt of the game's near a save point that must not be answered.
    pub ours: bool,
}

/// What `y_rule` looks at.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct YIn {
    /// `savepoint.sit = 1`: the rules before IMPL_ER26 (the game's own touch and rest stay reachable).
    pub sit: bool,
    /// `savepoint.menu` (0 = Y is always the game's).
    pub menu_key: bool,
    /// He stands in the Save-command capsule of a save point that exists (the game presents its grace) ...
    pub at_point: bool,
    /// ... or the game's own prompt is up within the guard radius of one (the game offers its action farther out
    /// than the capsule reaches).
    pub prompt_near: bool,
    /// A save point that exists is within the guard radius (`savepoint.guard`, m).
    /// (SV) ANY site of grace in reach counts: one hidden in combat, an undiscovered one, one the game hides.
    pub near: bool,
    pub discovered: bool,
    /// The rest rule (IMPL_ER21) lets him use it on this visit (the game's prompt was seen), or the rule is off.
    pub allowed: bool,
    /// Elden Ring's prompt is up now.
    pub prompt: bool,
    /// A menu of Elden Ring's has the pad; the game's rest runs; a command of the menu is being carried out.
    pub game_menu: bool,
    pub resting: bool,
    pub busy: bool,
    pub on_floor: bool,
    pub alive: bool,
}

/// THE Y RULE. Without sitting (`sit` false): at a discovered save point Y opens OUR menu directly, whether or not
/// the game shows its "Rest at site of grace" prompt at that moment; and a prompt of Elden Ring's that is up near a
/// save point (within `savepoint.guard`) is never answered by his Y, so neither its "Touch grace" (the kneel 26011 /
/// 68000) nor its rest (26031 / 68010 ...) can be started by the player. (Elden Ring's prompt cannot be hidden
/// selectively: it is one interface element for every action; it stays on screen and does nothing.) With `sit`: as before (the menu needs the game's prompt; at an undiscovered grace Y is the game's).
pub fn y_rule(i: YIn) -> YRule {
    let free = !i.game_menu && !i.resting && !i.busy;
    if i.sit {
        let offered = i.menu_key && i.at_point && i.discovered && i.prompt && free && i.on_floor && i.alive;
        return YRule { offered, ours: offered };
    }
    let offered = i.menu_key && (i.at_point || i.prompt_near) && i.discovered && i.allowed && free && i.on_floor && i.alive;
    // (`savepoint.menu = 0` gives Y back to the game altogether: its own rest and grace menu)
    // (without a prompt of the game's Y would not reach the game anyway: it stays Sora's, e.g. a situation command in a
    // fight beside a save point)
    // (SV, the user 2026-10-17: "I told you to REMOVE sites of grace so they are never accessible") the guard holds near EVERY
    // grace (`near`: hidden in combat, undiscovered, hidden by the game included) and whatever `savepoint.menu` says; only
    // `savepoint.sit = 1` (the debugging rules before IMPL_ER26) gives the game's touch and rest back. (The first route is
    // gracelock.rs: the game's grace prompts are switched off at their source; this is the second.)
    let guard = i.near && i.prompt && !i.game_menu && !i.resting;
    YRule { offered, ours: offered || guard }
}

/// (SP2, the user 2026-10-20: "There also needs to be a proper prompt for using the save point when inside it, currently
/// there is none") KH3'S "SAVE" REACTION COMMAND (the green event button over the command menu with the Y icon,
/// kh_host::hud `set_host_command(76)`): shown exactly while Y opens the save menu (`YRule::offered`: the leader stands in
/// the Save-command capsule of a save point that exists - not hidden in combat, not hidden by the game -, it is
/// discovered and usable, he is on the floor and alive, no flow runs), and no menu of ours is up, no cutscene runs;
/// `savepoint.prompt = 0` never. The leader is whoever leads (savepoint.rs tests the leader's capsule and life).
pub fn prompt_shows(offered: bool, menu_open: bool, cutscene: bool, key: bool) -> bool {
    key && offered && !menu_open && !cutscene
}

/// `savepoint.rest_events`: THE OLD RULE OF IMPL_ER26 (the game's real rest after a Save), kept as a key for debugging.
/// (IMPL_ER33, user decision 2026-10-13 "Remove it completely. Make the cutscenes happen automatically when you unlock
/// the correct amount of save points"): OFF BY DEFAULT; the story events are story.rs's (the table of the game's own
/// conditions, the rest made unseen at the moment a row is met).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RestEvents {
    /// DEFAULT (IMPL_ER33): a Save is never followed by a rest.
    Off,
    /// `savepoint.rest_events = sit` (the default of IMPL_ER26): the game's real rest follows a Save while such an
    /// event may still be pending (see `rest_after_save`).
    Sit,
    /// The game's real rest follows every Save.
    Always,
}

impl RestEvents {
    pub fn parse(value: Option<&str>) -> RestEvents {
        match value.map(str::trim) {
            Some("sit" | "1" | "on" | "true") => RestEvents::Sit,
            Some("always") => RestEvents::Always,
            _ => RestEvents::Off,
        }
    }
}

/// DOES THE GAME'S REAL REST FOLLOW THIS SAVE? `roundtable_found`: the Roundtable Hold's own site of grace is
/// discovered (`None`: its row or flag could not be read); `rested_here`: the game's rest was already made at this
/// save point in this session. The reason, for the log.
///
/// WHY THIS TEST: both early events (the accord, the invitation) are over once the Table of Lost Grace is on the map,
/// and that is a row of the game's own table (no event-flag number taken from memory). Which graces and which
/// progress the invitation needs is not known here, so until then every save point is rested at once (the first Save
/// there), which is where the game looks.
pub fn rest_after_save(mode: RestEvents, roundtable_found: Option<bool>, rested_here: bool) -> Option<&'static str> {
    match mode {
        RestEvents::Off => None,
        RestEvents::Always => Some("savepoint.rest_events = always"),
        RestEvents::Sit if rested_here => None,
        RestEvents::Sit => match roundtable_found {
            Some(true) => None,
            Some(false) => Some("the Roundtable Hold is not found yet: Melina's accord or her invitation may wait for a rest here"),
            None => Some("the Roundtable Hold's grace could not be read: taken as not found yet"),
        },
    }
}

/// The game's time-transition control (`CSEventWorldAreaTimeCtrl`) as read before it is used: the booleans as raw bytes.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CtrlSnapshot {
    pub target: [u32; 3],
    /// fade_transition, reset_world, reset_main_character, reset_magic_charges, restore_estus, show_clock,
    /// fade_out_requested, respawn_wait_flag.
    pub bools: [u8; 8],
    /// black_screen_time, clock_startup_delay_s, clock_move_time_s, clock_finish_delay_s, fade_out_time, fade_in_time,
    /// update_elapsed_time, black_screen_elapsed_time, total_elapsed_time, black_screen_timeout.
    pub floats: [f32; 10],
}

impl CtrlSnapshot {
    pub const FADE_OUT_REQUESTED: usize = 6;
    pub const RESPAWN_WAIT: usize = 7;

    /// A transition is under way.
    pub fn requested(&self) -> bool {
        self.bools[Self::FADE_OUT_REQUESTED] != 0
    }
}

/// IS THE OBJECT WHAT THE SDK'S LAYOUT SAYS (it was never used in this mod before)? Every boolean a 0 or 1, the
/// target a clock time, every duration finite, not negative and under an hour. `Err`: why it is not used.
pub fn ctrl_usable(s: &CtrlSnapshot) -> Result<(), &'static str> {
    if s.target[0] > 23 || s.target[1] > 59 || s.target[2] > 59 {
        return Err("its target time is no clock time");
    }
    if s.bools.iter().any(|b| *b > 1) {
        return Err("one of its switches is neither 0 nor 1");
    }
    if s.floats.iter().any(|f| !f.is_finite() || *f < 0.0 || *f > 3600.0) {
        return Err("one of its durations is no duration");
    }
    if s.requested() {
        return Err("a time transition of the game's is under way");
    }
    Ok(())
}

/// (IMPL_ER52, the user 2026-10-13: "remove the grace bit on the ground for every grace as well as it's collision and move
/// the save so it's in the same position on the ground as KH3") THE SAVE POINT ON THE GROUND. KH3 (0x140F9AA40, verified
/// at instruction level; kh_host::savepoint::ground_offset) traces 140 cm straight down from the actor root and spawns
/// its effects at the floor it meets (else 90 cm under the root). Here the root is `savepoint.height` above the grace's
/// character, which may stand on the grace's own mound: the trace is longer and ignores the hidden mound (graceflame.rs).
/// Sheet rows `savepoint.ground_*` of sora-er-host (host choices except where KH3 is named).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ground {
    /// `savepoint.ground_offset` (cm): added to the floor height found (0 = KH3: the effects' origin on the floor).
    pub offset: f32,
    /// `savepoint.ground_reach` (cm): how far the trace goes down from the root (KH3: 140 from a root 92 cm above its
    /// floor; here longer: the grace's character may stand on its mound, and the mound is not there for the trace).
    pub reach: f32,
    /// `savepoint.ground_miss` (cm): the effects' height relative to the root when no floor is met (KH3: -90).
    pub miss: f32,
}

impl Ground {
    pub const DEFAULT: Ground = Ground { offset: 0.0, reach: 250.0, miss: kh_host::savepoint::GROUND_MISS };

    /// One sheet row; false = not understood (ermap.rs lists it as a problem).
    pub fn apply_row(&mut self, id: &str, v: f32) -> bool {
        match id {
            "savepoint.ground_offset" if (-100.0..=200.0).contains(&v) => self.offset = v,
            "savepoint.ground_reach" if (10.0..=1000.0).contains(&v) => self.reach = v,
            "savepoint.ground_miss" if (-500.0..=0.0).contains(&v) => self.miss = v,
            _ => return false,
        }
        true
    }

    /// THE EFFECTS' HEIGHT RELATIVE TO THE ROOT (cm, Z): `floor` = the height (Z) the trace met, None = nothing within
    /// `reach`. KH3's rule (kh_host::savepoint::ground_offset) plus the host's offset.
    pub fn effect_z(&self, root_z: f32, floor: Option<f32>) -> f32 {
        let hit = floor.filter(|z| z.is_finite() && *z <= root_z + 1.0 && *z >= root_z - self.reach - 1.0).map(|z| glam::Vec3::new(0.0, 0.0, z));
        kh_host::savepoint::ground_offset(glam::Vec3::new(0.0, 0.0, root_z), hit, self.miss).z + self.offset
    }

    /// A new trace moves the effects only when it differs by more than this (cm): the effects are made anew then.
    pub const MOVE_AT: f32 = 2.0;
}

/// (IMPL_ER52) WHAT HAPPENS TO THE GRACE'S OWN MAP OBJECT (`AEG099_060`, the mound on the ground): `savepoint.kh` on,
/// `grace.hide_mound` (default 1 = its model is not drawn), `grace.mound_collision` (default 0 = Sora's casts pass
/// through it). Without the save points (`savepoint.kh = 0`) the game's object is left alone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MoundPlan {
    pub hide: bool,
    pub collide: bool,
}

pub fn mound_plan(savepoints: bool, hide_key: Option<bool>, collision_key: Option<bool>) -> MoundPlan {
    MoundPlan { hide: savepoints && hide_key != Some(false), collide: !savepoints || collision_key == Some(true) }
}

/// (LDI) THE LOAD-IN WINDOW as the save points see it (loadin.rs; sora.rs gives it every frame): the window this frame
/// and the frame before, `loadin.warm` (s), `loadin.arrive` (m), the leader's feet (Elden Ring metres).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LoadInPoint {
    pub window: bool,
    pub was: bool,
    pub warm: f32,
    pub arrive: f32,
    pub leader: glam::Vec3,
}

/// (LDI, the user 2026-10-22: save points should "load instantly when the game loads"; the picture: the save point still
/// in its swirl after a fast travel) ONE SAVE POINT'S LOAD-IN RULES:
/// - SETTLE: made in the load-in window (around him, under the loading screen), its first acting step takes his overlap as
///   where he already stands: no entry burst, no entry sound (savepoint.rs also does it every step of the window: the game
///   may still move him in the dark);
/// - THE GRACE HE ARRIVES AT: in the window, a DISCOVERED grace within `loadin.arrive` m of the leader stands at once,
///   before the game's flame is seen (MEASURED: the flame is seen 0.5 - 2.4 s after the new host, the picture fades in
///   about then); it keeps standing until the flame is seen, or `HOLD` s after the window if it never is (then the game's
///   word holds again: a hidden grace is hidden);
/// - WARM: when it comes to be shown, its idle effects are warmed to their steady state (`SavePoint::warm`) - in the window,
///   and for a save point coming into reach that was always there - but NOT when it APPEARS in play (the game hid it for
///   `GENUINE` s or more while he played: a boss's grace after the boss) and not when it comes back after a fight (KH3's
///   own activate).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Arrival {
    pub settle: bool,
    /// Shown as the grace he arrived at, before the game's flame was seen.
    pub standing: bool,
    after: f32,
    was_shown: bool,
    ever_shown: bool,
    /// Seconds the game hid it while he played (outside the window).
    hidden_play: f32,
}

impl Arrival {
    /// Seconds hidden in play after which a showing is an appearance (its effects start as KH3 starts them).
    pub const GENUINE: f32 = 2.0;
    /// Seconds after the window that the arrival grace stands without the game's flame.
    pub const HOLD: f32 = 3.0;

    /// A save point made now (`window`: in the load-in window).
    pub fn made(window: bool) -> Arrival {
        Arrival { settle: window, ..Arrival::default() }
    }

    /// Whether it is shown this frame: `rule` = the presence rule's answer (savepoint.rs), `present` / `hidden` = the
    /// game's word on its grace, `here` = discovered and within `loadin.arrive` of the leader (and the rule on).
    pub fn shown(&mut self, rule: bool, present: bool, hidden: bool, here: bool, window: bool, dt: f32) -> bool {
        let dt = if dt.is_finite() { dt.clamp(0.0, 0.5) } else { 0.0 };
        // (the hidden time is read and cleared by the next rising edge: `edge`)
        if hidden && !present && !window {
            self.hidden_play += dt;
        }
        if window && here && !present {
            (self.standing, self.after) = (true, 0.0);
        }
        if self.standing {
            if present || !here {
                self.standing = false;
            } else if !window {
                self.after += dt;
                if self.after > Self::HOLD {
                    self.standing = false;
                }
            }
        }
        rule || self.standing
    }

    /// The final word of the frame (after the combat rule). On the rising edge: `Some(warm)`, its idle effects to be
    /// warmed at their spawn or not.
    pub fn edge(&mut self, shown: bool, window: bool) -> Option<bool> {
        let rose = shown && !self.was_shown;
        self.was_shown = shown;
        if !rose {
            return None;
        }
        let warm = window || (!self.ever_shown && self.hidden_play < Self::GENUINE);
        (self.ever_shown, self.hidden_play) = (true, 0.0);
        Some(warm)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 1.0 / 60.0;

    /// (LDI) FAST TRAVEL TO A GRACE: the save point is made in the window (settle), the game's flame is not seen yet
    /// (Unknown, then Hidden while the area streams in): it stands at once as the grace he arrives at, warmed; the flame
    /// seen later changes nothing; no second edge.
    #[test]
    fn the_grace_he_arrives_at_stands_at_once_and_warmed() {
        let mut a = Arrival::made(true);
        assert!(a.settle);
        // frame 1: not known yet (rule says no), discovered and 1.6 m from him (here)
        assert!(a.shown(false, false, false, true, true, DT));
        assert_eq!(a.edge(true, true), Some(true));
        // the game hides it while its area streams in, in the window: it keeps standing
        for _ in 0..120 {
            assert!(a.shown(false, false, true, true, true, DT));
            assert_eq!(a.edge(true, true), None);
        }
        // the flame is seen (after the window too): the rule's word, no new edge
        for _ in 0..300 {
            assert!(a.shown(true, true, false, true, false, DT));
            assert_eq!(a.edge(true, false), None);
        }
        assert!(!a.standing);
    }

    /// The arrival hold ends: a grace the game keeps hiding after the window goes HOLD s later (the game's word holds);
    /// an undiscovered / far grace never stands by this rule; the rule off (here false) changes nothing.
    #[test]
    fn a_grace_the_game_keeps_hiding_goes_after_the_hold() {
        let mut a = Arrival::made(true);
        assert!(a.shown(false, false, true, true, true, DT));
        let frames = 1 + (0..1000).take_while(|_| a.shown(false, false, true, true, false, DT)).count();
        assert!((180..=182).contains(&frames), "{frames}");
        let mut far = Arrival::made(true);
        assert!(!far.shown(false, false, false, false, true, DT));
        assert!(!far.shown(false, false, true, false, true, DT));
    }

    /// WARM OR NOT: coming into reach in play (always there) = warmed; shown again after a fight = KH3's own start; a
    /// grace that APPEARS in play (hidden 2 s or more, then shown: a boss's grace) = its own start; a hidden flicker under a
    /// second = still warmed.
    #[test]
    fn warm_only_what_was_always_there() {
        let mut reach = Arrival::made(false);
        assert!(!reach.settle);
        reach.shown(true, true, false, false, false, DT);
        assert_eq!(reach.edge(true, false), Some(true));
        // a fight hides it, then it is back: no warm
        assert_eq!(reach.edge(false, false), None);
        assert_eq!(reach.edge(true, false), Some(false));
        // the boss's grace: hidden 5 s in play, then the flame
        let mut boss = Arrival::made(false);
        for _ in 0..300 {
            assert!(!boss.shown(false, false, true, false, false, DT));
            assert_eq!(boss.edge(false, false), None);
        }
        assert!(boss.shown(true, true, false, false, false, DT));
        assert_eq!(boss.edge(true, false), Some(false));
        // a short flicker of "hidden" before the flame (a grace coming into reach): warmed
        let mut flick = Arrival::made(false);
        for _ in 0..30 {
            flick.shown(false, false, true, false, false, DT);
            flick.edge(false, false);
        }
        flick.shown(true, true, false, false, false, DT);
        assert_eq!(flick.edge(true, false), Some(true));
        // in the window: always warmed, even after a long hide (the load)
        let mut load = Arrival::made(true);
        for _ in 0..300 {
            load.shown(false, false, true, false, true, DT);
            load.edge(false, true);
        }
        load.shown(true, true, false, false, true, DT);
        assert_eq!(load.edge(true, true), Some(true));
    }

    #[test]
    fn the_save_point_lies_on_the_floor_under_its_root() {
        let g = Ground::DEFAULT;
        // the root 92 cm above the grace's character; the floor 92 cm down: the effects 92 cm under the root (KH3)
        assert_eq!(g.effect_z(192.0, Some(100.0)), -92.0);
        // the character stands on a 40 cm mound that is gone for the trace: the floor 132 cm down (KH3's 140 would reach it too)
        assert_eq!(g.effect_z(192.0, Some(60.0)), -132.0);
        // deeper than KH3's 140 but within the host's reach
        assert_eq!(g.effect_z(192.0, Some(0.0)), -192.0);
        // nothing within reach, or a reading outside the trace: KH3's 90 cm under the root
        assert_eq!(g.effect_z(192.0, None), -90.0);
        assert_eq!(g.effect_z(192.0, Some(-500.0)), -90.0);
        assert_eq!(g.effect_z(192.0, Some(f32::NAN)), -90.0);
        // the host's offset lifts both
        let up = Ground { offset: 3.0, ..g };
        assert_eq!(up.effect_z(192.0, Some(100.0)), -89.0);
        assert_eq!(up.effect_z(192.0, None), -87.0);
        // sheet rows
        let mut r = Ground::DEFAULT;
        assert!(r.apply_row("savepoint.ground_offset", 5.0) && r.apply_row("savepoint.ground_reach", 140.0) && r.apply_row("savepoint.ground_miss", -90.0));
        assert_eq!(r, Ground { offset: 5.0, reach: 140.0, miss: -90.0 });
        assert!(!r.apply_row("savepoint.ground_miss", 10.0));
        assert!(!r.apply_row("savepoint.ground_other", 1.0));
    }

    #[test]
    fn the_mound_is_hidden_and_passable_by_default_and_both_can_be_restored() {
        assert_eq!(mound_plan(true, None, None), MoundPlan { hide: true, collide: false });
        assert_eq!(mound_plan(true, Some(false), None), MoundPlan { hide: false, collide: false });
        assert_eq!(mound_plan(true, None, Some(true)), MoundPlan { hide: true, collide: true });
        assert_eq!(mound_plan(true, Some(true), Some(false)), MoundPlan { hide: true, collide: false });
        // without the save points nothing of the game's is touched
        assert_eq!(mound_plan(false, Some(true), Some(false)), MoundPlan { hide: false, collide: true });
    }

    #[test]
    fn a_save_point_unlocks_once_when_he_runs_into_its_capsule() {
        let mut u = Unlock::default();
        // approaching: outside the capsule nothing happens, however long
        assert!((0..600).all(|_| u.step(true, true, Some(false), false, DT) == UnlockStep::Nothing));
        // the frame he enters: one write
        assert_eq!(u.step(true, true, Some(false), true, DT), UnlockStep::Write);
        // the flag reads back as set: never again, inside or outside
        assert!((0..600).all(|k| u.step(true, true, Some(true), k % 2 == 0, DT) == UnlockStep::Nothing));
        assert_eq!(u.tries(), 1);
        // a discovered grace he walks into: nothing
        let mut d = Unlock::default();
        assert!((0..120).all(|_| d.step(true, true, Some(true), true, DT) == UnlockStep::Nothing));
        assert_eq!(d.tries(), 0);
    }

    #[test]
    fn a_hidden_or_unknown_grace_is_never_unlocked_and_the_key_switches_it_off() {
        // the game hides the grace (a boss's before the boss is defeated): standing where it will be does nothing
        let mut hidden = Unlock::default();
        assert!((0..600).all(|_| hidden.step(true, false, Some(false), true, DT) == UnlockStep::Nothing));
        assert_eq!(hidden.tries(), 0);
        // the flag cannot be read: nothing is written blind
        let mut unknown = Unlock::default();
        assert!((0..600).all(|_| unknown.step(true, true, None, true, DT) == UnlockStep::Nothing));
        // savepoint.autounlock = 0 (or savepoint.sit = 1)
        let mut off = Unlock::default();
        assert!((0..600).all(|_| off.step(false, true, Some(false), true, DT) == UnlockStep::Nothing));
        // the boss dies while he stands there: the grace appears, the very next frame unlocks it
        assert_eq!(hidden.step(true, true, Some(false), true, DT), UnlockStep::Write);
    }

    #[test]
    fn a_write_that_does_not_stick_is_tried_three_times_a_second_apart_then_given_up_once() {
        let mut u = Unlock::default();
        let mut writes = Vec::new();
        let mut gave_up = 0;
        for frame in 0..1200 {
            match u.step(true, true, Some(false), true, DT) {
                UnlockStep::Write => writes.push(frame),
                UnlockStep::GaveUp => gave_up += 1,
                UnlockStep::Nothing => {}
            }
        }
        assert_eq!(writes.len(), Unlock::TRIES as usize);
        assert!(writes.windows(2).all(|w| (w[1] - w[0]) as f32 * DT >= Unlock::RETRY - 0.02), "{writes:?}");
        assert_eq!(gave_up, 1);
    }

    fn at_discovered() -> YIn {
        YIn { sit: false, menu_key: true, at_point: true, prompt_near: false, near: true, discovered: true, allowed: true, prompt: false, game_menu: false, resting: false, busy: false, on_floor: true, alive: true }
    }

    #[test]
    fn y_at_a_save_point_is_ours_with_or_without_the_games_prompt() {
        // discovered, in the capsule, the game's "Rest" prompt NOT up: our menu all the same
        assert_eq!(y_rule(at_discovered()), YRule { offered: true, ours: true });
        assert_eq!(y_rule(YIn { prompt: true, ..at_discovered() }), YRule { offered: true, ours: true });
        // outside the capsule but inside the game's prompt: our menu, not the game's rest
        assert_eq!(y_rule(YIn { at_point: false, prompt_near: true, prompt: true, ..at_discovered() }), YRule { offered: true, ours: true });
        // near it without a prompt and outside the capsule: no menu; Y is Sora's (the game would not get it anyway)
        assert_eq!(y_rule(YIn { at_point: false, ..at_discovered() }), YRule { offered: false, ours: false });
        // ... and with a prompt of the game's that is not a discovered grace's to answer: kept from the game
        assert_eq!(y_rule(YIn { at_point: false, prompt: true, discovered: false, ..at_discovered() }), YRule { offered: false, ours: true });
        // not discovered (the frame before the unlock is read back): "Touch grace" cannot be pressed
        assert_eq!(y_rule(YIn { discovered: false, prompt: true, ..at_discovered() }), YRule { offered: false, ours: true });
        // in the air over it, or the rest rule refuses (enemies alerted): no menu, never the game's rest
        assert_eq!(y_rule(YIn { on_floor: false, prompt: true, ..at_discovered() }), YRule { offered: false, ours: true });
        assert_eq!(y_rule(YIn { allowed: false, ..at_discovered() }), YRule { offered: false, ours: false });
        assert_eq!(y_rule(YIn { allowed: false, prompt: true, ..at_discovered() }), YRule { offered: false, ours: true });
        // far from every save point: not ours
        assert_eq!(y_rule(YIn { at_point: false, near: false, ..at_discovered() }), YRule { offered: false, ours: false });
        // a menu of the game's is up (its map, its grace menu in the fallback rest): Y is the game's there
        assert_eq!(y_rule(YIn { game_menu: true, ..at_discovered() }), YRule { offered: false, ours: false });
        assert_eq!(y_rule(YIn { resting: true, ..at_discovered() }), YRule { offered: false, ours: false });
        // savepoint.menu = 0: no menu, and without a prompt Y is Sora's
        assert_eq!(y_rule(YIn { menu_key: false, ..at_discovered() }), YRule { offered: false, ours: false });
        // (SV) ... but a prompt of the game's near a grace is never answered, whatever savepoint.menu says
        assert_eq!(y_rule(YIn { menu_key: false, prompt: true, ..at_discovered() }), YRule { offered: false, ours: true });
    }

    /// (SV, the user 2026-10-17 01:00) THE GAME'S GRACE PROMPT IS NEVER ANSWERED: in combat (the save point hidden: not
    /// in its capsule, no menu offered), at an undiscovered grace, at one the game hides, on the ground or in the air.
    #[test]
    fn the_games_grace_prompt_is_never_answered_in_any_state() {
        let combat = YIn { at_point: false, prompt_near: false, near: true, discovered: false, allowed: false, prompt: true, ..at_discovered() };
        assert_eq!(y_rule(combat), YRule { offered: false, ours: true });
        for discovered in [false, true] {
            for allowed in [false, true] {
                for at_point in [false, true] {
                    for on_floor in [false, true] {
                        for menu_key in [false, true] {
                            for busy in [false, true] {
                                let i = YIn { discovered, allowed, at_point, on_floor, menu_key, busy, ..combat };
                                assert!(y_rule(i).ours, "{i:?}");
                            }
                        }
                    }
                }
            }
        }
        // a menu of the game's or its rest (only ever ours to start: the story's driven rest) keeps the pad
        assert!(!y_rule(YIn { game_menu: true, ..combat }).ours);
        // far from every grace: the game's prompt is another thing's (a door, an item): the game's
        assert!(!y_rule(YIn { near: false, ..combat }).ours);
    }

    /// (SP2) THE SAVE PROMPT: up while he stands in the save point and Y opens the menu; gone when he leaves, when the save
    /// point is hidden in combat (its capsule no longer counts: `at_point` false, the rest rule's latch dropped), in the
    /// air, dead, while a menu / flow / cutscene runs, and with the key off.
    #[test]
    fn the_save_prompt_shows_inside_and_goes_when_he_leaves_or_combat_hides_the_point() {
        let shows = |i: YIn, menu: bool, cut: bool| prompt_shows(y_rule(i).offered, menu, cut, true);
        assert!(shows(at_discovered(), false, false), "inside a usable save point");
        assert!(!shows(YIn { at_point: false, ..at_discovered() }, false, false), "he left");
        assert!(!shows(YIn { at_point: false, allowed: false, ..at_discovered() }, false, false), "combat: the save point is hidden");
        assert!(!shows(YIn { at_point: false, near: true, prompt: true, discovered: false, allowed: false, ..at_discovered() }, false, false), "combat beside a grace whose game prompt is up");
        assert!(!shows(YIn { on_floor: false, ..at_discovered() }, false, false), "in the air over it");
        assert!(!shows(YIn { alive: false, ..at_discovered() }, false, false), "the leader is down");
        assert!(!shows(YIn { busy: true, ..at_discovered() }, false, false), "a command of the menu runs");
        assert!(!shows(YIn { game_menu: true, ..at_discovered() }, false, false));
        assert!(!shows(at_discovered(), true, false), "our save menu is up (it took the Y)");
        assert!(!shows(at_discovered(), false, true), "a cutscene");
        assert!(!prompt_shows(true, false, false, false), "savepoint.prompt = 0");
        // the moment the unlock is read back (undiscovered -> discovered) it comes up
        assert!(!shows(YIn { discovered: false, ..at_discovered() }, false, false));
        // back in after the fight: up again
        assert!(shows(at_discovered(), false, false));
    }

    #[test]
    fn with_sitting_restored_y_is_as_before() {
        let sit = YIn { sit: true, ..at_discovered() };
        // the menu needs the game's prompt; without it Y is not ours
        assert_eq!(y_rule(sit), YRule { offered: false, ours: false });
        assert_eq!(y_rule(YIn { prompt: true, ..sit }), YRule { offered: true, ours: true });
        // an undiscovered grace: Y is the game's ("Touch grace")
        assert_eq!(y_rule(YIn { prompt: true, discovered: false, ..sit }), YRule { offered: false, ours: false });
    }

    #[test]
    fn the_real_rest_follows_a_save_only_while_an_early_story_event_may_wait() {
        use RestEvents::*;
        // (IMPL_ER33) off unless asked for: a Save is never followed by a rest
        assert_eq!(RestEvents::parse(None), Off);
        assert!(rest_after_save(RestEvents::parse(None), Some(false), false).is_none());
        assert_eq!(RestEvents::parse(Some("sit")), Sit);
        assert_eq!(RestEvents::parse(Some("0")), Off);
        assert_eq!(RestEvents::parse(Some("always")), Always);
        // before the Roundtable Hold is found: the first Save at each save point rests, a second one there does not
        assert!(rest_after_save(Sit, Some(false), false).is_some());
        assert!(rest_after_save(Sit, Some(false), true).is_none());
        // unreadable = not found yet (the safe side)
        assert!(rest_after_save(Sit, None, false).is_some());
        // found: never again
        assert!(rest_after_save(Sit, Some(true), false).is_none());
        // the two other settings
        assert!(rest_after_save(Off, Some(false), false).is_none());
        assert!(rest_after_save(Always, Some(true), true).is_some());
    }

    #[test]
    fn the_time_control_is_used_only_when_it_looks_like_the_sdk_says() {
        let idle = CtrlSnapshot { target: [12, 0, 0], bools: [0; 8], floats: [1.5, 0.0, 0.0, 0.0, 0.75, 0.5, 0.0, 0.0, 0.0, 10.0] };
        assert_eq!(ctrl_usable(&idle), Ok(()));
        assert_eq!(ctrl_usable(&CtrlSnapshot::default()), Ok(()));
        assert!(ctrl_usable(&CtrlSnapshot { target: [0x3f80_0000, 0, 0], ..idle }).is_err());
        let mut b = idle;
        b.bools[2] = 0x40;
        assert!(ctrl_usable(&b).is_err());
        let mut f = idle;
        f.floats[4] = f32::NAN;
        assert!(ctrl_usable(&f).is_err());
        f.floats[4] = -1.0e30;
        assert!(ctrl_usable(&f).is_err());
        // busy with a transition of the game's own (or ours): not asked twice
        let mut r = idle;
        r.bools[CtrlSnapshot::FADE_OUT_REQUESTED] = 1;
        assert!(r.requested());
        assert!(ctrl_usable(&r).is_err());
    }
}
