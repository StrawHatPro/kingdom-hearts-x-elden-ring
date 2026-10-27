//! (ANIMS 2026-10-23) EVERY WAY OFF A LADDER, AND OUT OF ANY OF THE BODY'S OWN ACTIONS, ENDS THE INTERACTION.
//!
//! THE USER: "also jumping off a ladder doesn't properly end the interaction, so it softlocks you as you can no longer
//! interact with anything else".
//!
//! (LADDER2 2026-10-25: nobody climbs any more. Y at a ladder lets the body go at once, ladder.rs `LetGo`, with the same
//! `W_LadderDrop`: these rules still end whatever the game leaves behind.)
//!
//! WHAT WENT WRONG (read in the code; the user's log of it was not kept): B on a ladder fired the body's behaviour event
//! `W_LadderDrop` straight into its Havok graph (the old jump off). The graph leaves its ladder states, but the C++
//! ladder module (`CSChrLadderModule`: `state`, `ladder_handle`) is written by the ladder states' own script, which no
//! longer runs: its state can stay at the rung's value (e.g. 9 IdleLeft) with the body standing or falling in ordinary
//! animations. Everything of ours believed that state: `Ladder` said "on a ladder" for ever, so Sora FOLLOWED the body
//! (`Follow::Action`): the pad was the ladder's (the game saw the stick only, Y and every other button hidden: ladder.rs
//! `Pad`), the jump off's wait gave up at 1.5 s and followed the "ladder" again, and the game itself, its module still
//! attached to the ladder, offered no prompt. Nothing ever reset it but a load.
//!
//! THE RULES NOW (pure, below; sora.rs applies them once a frame):
//! - THE GAME'S OWN STATE DECIDES, CROSS-CHECKED (`LadderTruth`): a ladder state of 0 or more while the body has not
//!   played one of the ladder animations for `STALE_S` is STALE: believed -1 (the follow ends, the jump off sees the
//!   let-go, the pad is his again) and the module is REPAIRED once (state -1, the ladder handle emptied: `ladder.repair = 0`
//!   switches the write off, the belief stays). A ladder animation without a ladder state that outlives the follow's
//!   3 s while the leader stands on a floor (a let-go whose fall never ended) is NUDGED once (the body's `W_Idle`).
//! - THE SAFETY NET (`BusyNet`): our side "busy" (on a ladder, a ladder released, a jump off waiting, an action held)
//!   while the body has been in plain locomotion for `NET_S`: every ladder / action state of ours is cleared.
//! - THE ACTIONS' CAP (`OwnCap`): a door / fog / lever follow (`own.move.*`, `own.unlisted`) of one animation longer than
//!   `own.max_s` (default 20 s; the longest measured is the double door's 6.83 s) is spent until the animation changes.
//! - A death, a load or a warp clears every ladder state of ours at once (sora.rs).

/// What the body shows of its ladder this frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Seen {
    /// The ladder module's state (-1 none).
    pub state: i8,
    /// The body plays one of the ladder animations (`own.ladder_anims`, 28000-28999).
    pub ladder_anim: bool,
    /// The leader stands on a floor.
    pub lead_on_floor: bool,
}

/// What the game's module needs this frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Fix {
    None,
    /// The module still says "ladder" with no ladder animation for `STALE_S`: write its state -1, empty its handle.
    Repair,
    /// A ladder animation without a ladder state for `STUCK_S` with the leader standing: ask the body's graph for idle.
    Nudge,
}

/// The ladder state as it is to be believed (pure).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LadderTruth {
    stale_for: f32,
    stuck_for: f32,
    repaired: bool,
    nudged: bool,
    /// Stale episodes and nudges so far (the log).
    pub repairs: u32,
    pub nudges: u32,
}

impl LadderTruth {
    /// A ladder state without a ladder animation this long is stale (the longest gap between two ladder animations seen
    /// in the user's climbs is a blend of a few frames).
    pub const STALE_S: f32 = 0.6;
    /// A ladder animation without a state this long with the leader standing (the follow's own cap is 3 s).
    pub const STUCK_S: f32 = 3.5;

    pub fn frame(&mut self, seen: Seen, dt: f32) -> (i8, Fix) {
        let mut fix = Fix::None;
        if seen.state >= 0 && !seen.ladder_anim {
            self.stale_for += dt;
        } else {
            self.stale_for = 0.0;
            self.repaired = false;
        }
        let stale = self.stale_for >= Self::STALE_S;
        if stale && !self.repaired {
            self.repaired = true;
            self.repairs += 1;
            fix = Fix::Repair;
        }
        if seen.state < 0 && seen.ladder_anim && seen.lead_on_floor {
            self.stuck_for += dt;
        } else {
            self.stuck_for = 0.0;
        }
        if !seen.ladder_anim {
            self.nudged = false;
        }
        if self.stuck_for >= Self::STUCK_S && !self.nudged && fix == Fix::None {
            self.nudged = true;
            self.nudges += 1;
            fix = Fix::Nudge;
        }
        (if stale { -1 } else { seen.state }, fix)
    }

    pub fn stale(&self) -> bool {
        self.stale_for >= Self::STALE_S
    }
}

/// The safety net (pure): our side busy while the body is in plain locomotion for `NET_S` -> clear.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BusyNet {
    for_s: f32,
    /// Times it fired (the log).
    pub fired: u32,
}

impl BusyNet {
    pub const NET_S: f32 = 1.0;

    /// True on the frame our states are to be cleared.
    pub fn frame(&mut self, busy: bool, body_normal: bool, dt: f32) -> bool {
        if !(busy && body_normal) {
            self.for_s = 0.0;
            return false;
        }
        self.for_s += dt;
        if self.for_s >= Self::NET_S {
            self.for_s = 0.0;
            self.fired += 1;
            return true;
        }
        false
    }
}

/// The actions' cap (pure): one listed animation followed longer than `max_s` is spent until the animation changes.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct OwnCap {
    anim: i32,
    for_s: f32,
    spent: bool,
}

impl OwnCap {
    pub const DEFAULT_MAX_S: f32 = 20.0;

    /// `listed`: the body's animation is a follow row this frame. Returns whether it may still be followed; the second
    /// value is true on the frame it was spent (the log).
    pub fn frame(&mut self, anim: i32, listed: bool, max_s: f32, dt: f32) -> (bool, bool) {
        if !listed || anim != self.anim {
            self.anim = anim;
            self.for_s = 0.0;
            self.spent = false;
            return (listed, false);
        }
        self.for_s += dt;
        if !self.spent && self.for_s > max_s.max(1.0) {
            self.spent = true;
            return (false, true);
        }
        (!self.spent, false)
    }
}

/// (CHESTFIX, the user 2026-10-27: "opening a chest as Donald completely broke Sora, he stopped moving and even swapping
/// to him from the party menu I couldn't move as him") THE NET ON SORA'S OWN SIMULATION (pure): his kh-core state has
/// stood in Idle for `NET_S` with the stick given to it pushed past the run threshold (the pad's while he leads, AI Sora's
/// while a friend does), on the floor, without moving, while nothing of ours or the game's holds him (no follow of the
/// body, no action hold, no leap, no menu, no boss finish, alive, no shotlock aim open): in Idle the stick starts his run
/// on the very next step, so something left in his state refuses every request. The host then makes him afresh where he
/// stands (`Sora::teleport`: a new state machine, his status, magic and menu kept), as after a warp. The user's run: a
/// warp of AI Sora in his shotlock's ShootFlow left the aim mode open for 250 s (kh-core shotlock.rs `close_dropped`
/// fixes that cause; this net catches any other).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct StillNet {
    for_s: f32,
    moved: f32,
    /// Times it fired (the log).
    pub fired: u32,
}

impl StillNet {
    pub const NET_S: f32 = 2.0;
    /// The stick that starts a run from Idle (kh-core run.rs: magnitude above 0.5).
    pub const STICK: f32 = 0.55;
    /// Moving less than this (cm) over the window counts as not moving.
    pub const STILL_CM: f32 = 10.0;

    /// One step of his simulation. `idle`: his state is Idle; `stick`: the magnitude of the stick he was given; `moved`:
    /// how far (cm, flat) the step carried him; `free`: nothing holds him (see above). True on the step he is to be freed.
    pub fn step(&mut self, idle: bool, stick: f32, moved: f32, on_floor: bool, free: bool, dt: f32) -> bool {
        if !(idle && on_floor && free && stick >= Self::STICK) {
            (self.for_s, self.moved) = (0.0, 0.0);
            return false;
        }
        self.for_s += dt;
        self.moved += moved;
        if self.moved >= Self::STILL_CM {
            (self.for_s, self.moved) = (0.0, 0.0);
            return false;
        }
        if self.for_s >= Self::NET_S {
            (self.for_s, self.moved) = (0.0, 0.0);
            self.fired += 1;
            return true;
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ladder::{LetGo, LetStep, Watch};

    /// (CHESTFIX) The user's frozen Sora: Idle, the stick full, not moving, nothing holding him -> freed after 2 s, again
    /// 2 s later if still frozen; a stick under the run threshold, a step that moves him, anything that holds him (a
    /// follow, a menu, the aim) or another state (Run against a wall, an action) never fires.
    #[test]
    fn chestfix_the_still_net_frees_a_frozen_sora() {
        let mut net = StillNet::default();
        let fired: Vec<usize> = (0..300).filter(|_| net.step(true, 1.0, 0.0, true, true, DT)).collect();
        assert_eq!(fired.len(), 2, "{fired:?}");
        assert!(((fired[0] + 1) as f32 * DT - StillNet::NET_S).abs() < 0.02);
        assert_eq!(net.fired, 2);
        let mut quiet = StillNet::default();
        assert!((0..300).all(|_| !quiet.step(true, 0.3, 0.0, true, true, DT)), "a stick under the run threshold");
        assert!((0..300).all(|_| !quiet.step(true, 1.0, 0.0, true, false, DT)), "something holds him");
        assert!((0..300).all(|_| !quiet.step(false, 1.0, 0.0, true, true, DT)), "Run against a wall / an action");
        assert!((0..300).all(|_| !quiet.step(true, 1.0, 0.0, false, true, DT)), "in the air");
        assert!((0..300).all(|_| !quiet.step(true, 1.0, 5.0, true, true, DT)), "he moves");
        assert_eq!(quiet.fired, 0);
    }

    const DT: f32 = 1.0 / 60.0;

    /// (LADDER2) Our whole ladder side as sora.rs runs it, frame by frame (pure): the truth, the start, the let-go, the net.
    /// `busy()` is what would keep the body from the leader and the interactions away.
    #[derive(Default)]
    struct Side {
        truth: LadderTruth,
        watch: Watch,
        go: LetGo,
        net: BusyNet,
        repaired_module: bool,
        dropped: bool,
        forced: bool,
        starts: u32,
    }

    impl Side {
        fn frame(&mut self, state: i8, ladder_anim: bool, allowed: bool, on_floor: bool) -> bool {
            let (believed, fix) = self.truth.frame(Seen { state, ladder_anim, lead_on_floor: on_floor }, DT);
            if fix == Fix::Repair {
                self.repaired_module = true;
            }
            if !allowed {
                self.go.end();
            }
            if self.watch.frame(believed, ladder_anim, allowed, DT) {
                self.starts += 1;
                self.go.start();
            }
            match self.go.frame(believed, DT) {
                LetStep::Drop => self.dropped = true,
                LetStep::Force => self.forced = true,
                _ => {}
            }
            self.go.idle_due(ladder_anim, DT);
            if self.go.over(ladder_anim) {
                self.go.end();
            }
            if self.net.frame(self.go.active, believed < 0 && !ladder_anim, DT) {
                self.go.end();
            }
            self.busy()
        }

        fn busy(&self) -> bool {
            self.go.active
        }

        /// Y at the ladder: the approach (0.2 s), then the game's StartBottom.
        fn y(&mut self) {
            for _ in 0..12 {
                self.frame(-1, true, true, true);
            }
            self.frame(0, true, true, true);
            assert!(self.dropped && self.busy());
        }

        /// Seconds until our side is no longer busy (None: still busy after 10 s).
        fn settles(&mut self, mut each: impl FnMut(&mut Side, usize)) -> Option<f32> {
            (0..600).position(|k| {
                each(self, k);
                !self.busy()
            }).map(|k| k as f32 * DT)
        }
    }

    /// Y at a ladder, the game answers the drop (state -1, its fall animation a few frames, then its idle): free at once,
    /// and the next ladder starts again.
    #[test]
    fn y_at_a_ladder_lets_the_body_go_and_ends_the_interaction() {
        let mut s = Side::default();
        s.y();
        let t = s.settles(|s, k| {
            s.frame(-1, k < 20, true, k > 30);
        });
        assert!(t.unwrap() < 0.5, "{t:?}");
        assert!(!s.repaired_module && !s.forced);
        s.frame(-1, false, true, true);
        s.dropped = false;
        s.y();
        assert_eq!(s.starts, 2, "the next ladder");
    }

    /// THE OLD SOFTLOCK (the user: "jumping off a ladder doesn't properly end the interaction"): after the drop the module
    /// keeps its state 9 while the body plays none of the ladder animations: believed over, the module repaired, free.
    #[test]
    fn a_let_go_whose_ladder_state_stays_ends_the_interaction() {
        let mut s = Side::default();
        s.y();
        let t = s.settles(|s, k| {
            s.frame(9, k < 6, true, k > 40);
        });
        assert!(t.expect("still busy 10 s after the let-go: the softlock") < 1.6, "{t:?}");
        assert!(s.repaired_module, "the game's module is told the ladder is over");
    }

    /// The game keeps its body on the ladder (its ladder animation plays on): written free, then over; never for ever.
    #[test]
    fn a_body_the_game_keeps_is_written_free() {
        let mut s = Side::default();
        s.y();
        let t = s.settles(|s, _| {
            s.frame(9, true, true, false);
        });
        assert!(s.forced, "the module written free");
        assert!(t.unwrap() < LetGo::FORCE_S + LetGo::OVER_S + 0.2, "{t:?}");
    }

    /// An approach the game never turns into a ladder state: free after the approach's limit.
    #[test]
    fn an_approach_without_a_state_ends() {
        let mut s = Side::default();
        let t = s.settles(|s, k| {
            s.frame(-1, k < 300, true, true);
        });
        assert!(t.unwrap() < LetGo::APPROACH_MAX_S + LetGo::OVER_S + 0.2, "{t:?}");
    }

    /// Death or a load / a warp at a ladder: the rule is not allowed that frame -> off at once.
    #[test]
    fn death_or_a_load_ends_it() {
        let mut s = Side::default();
        s.y();
        s.frame(9, true, false, false);
        assert!(!s.busy());
        assert!((0..60).all(|_| !s.frame(-1, false, true, true)));
    }

    /// A ladder animation stuck without its state while he stands: the follow ends at 3 s, the body is nudged once.
    #[test]
    fn a_stuck_ladder_animation_is_nudged_once() {
        let mut t = LadderTruth::default();
        let fixes: Vec<Fix> = (0..400).map(|_| t.frame(Seen { state: -1, ladder_anim: true, lead_on_floor: true }, DT).1).collect();
        assert_eq!(fixes.iter().filter(|f| **f == Fix::Nudge).count(), 1);
        assert!(!fixes.contains(&Fix::Repair));
        // in the air (a let-go still falling): never
        let mut a = LadderTruth::default();
        assert!((0..400).all(|_| a.frame(Seen { state: -1, ladder_anim: true, lead_on_floor: false }, DT).1 == Fix::None));
    }

    /// A real climb is never repaired: the blend frames between ladder animations are short.
    #[test]
    fn a_real_climb_is_believed() {
        let mut t = LadderTruth::default();
        for k in 0..2000 {
            let anim = k % 50 > 3; // 4 frames of something else every 50
            let (s, f) = t.frame(Seen { state: 3, ladder_anim: anim, lead_on_floor: false }, DT);
            assert_eq!((s, f), (3, Fix::None));
        }
    }

    /// The net: busy while the body is plain for 1 s -> cleared; busy while the body acts -> never.
    #[test]
    fn the_net_clears_a_busy_side_over_plain_locomotion_only() {
        let mut n = BusyNet::default();
        let at = (0..120).position(|_| n.frame(true, true, DT)).unwrap();
        assert!(((at + 1) as f32 * DT - BusyNet::NET_S).abs() < 0.02);
        let mut m = BusyNet::default();
        assert!((0..6000).all(|_| !m.frame(true, false, DT)));
        assert!((0..6000).all(|_| !m.frame(false, true, DT)));
    }

    /// The actions' cap: a door followed for at most `max_s`, then spent until the animation changes.
    #[test]
    fn a_door_or_fog_follow_is_capped() {
        let mut c = OwnCap::default();
        let ok: Vec<bool> = (0..60 * 25).map(|_| c.frame(60030, true, 20.0, DT).0).collect();
        let last = ok.iter().rposition(|b| *b).unwrap();
        assert!(((last + 1) as f32 * DT - 20.0).abs() < 0.05, "{last}");
        assert!(!c.frame(60030, true, 20.0, DT).0);
        assert!(!c.frame(0, false, 20.0, DT).0);
        assert!(c.frame(60030, true, 20.0, DT).0, "a new door is followed");
        // the measured lengths are never cut
        let mut f = OwnCap::default();
        assert!((0..(6.83 * 60.0) as usize).all(|_| f.frame(60030, true, 20.0, DT).0));
    }
}
