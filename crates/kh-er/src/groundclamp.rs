//! (THE USER 2026-11-02: "I also noticed an enemy clipped into the ground fully after party member Sora attacked him")
//! THE GROUND RULE OF EVERY ENEMY WE MOVE, pure (puppet.rs is the game half: the cast, the writes).
//!
//! What the log showed (kh-er.log 311.6 - 318.3 s): AI Sora's Kingdom Key shotlock (`SF_KCSHOT`, four Sidespin hits) spun a
//! c4311 away (`SpunAway`: kh-react flies the body with its gravity off, and a floor its sweep skims keeps only the
//! horizontal part of the move, so ground rising under the spin is flown INTO); the body came out of the spin under the
//! surface, no sweep from inside the ground found a floor again, and it fell through the world for 2.7 s (its bones 36 m
//! under its node) until it left the area the targets are read in. Nothing checked where the written place was against
//! the ground. Besides, the Elden Ring places kept for a body (where it was taken, what was last written) were never
//! moved with the game's physics origin, and the reaction's KH3 place was never re-read when the KH3 frame moved under
//! it: one log showed a body written 58 m up and 87 m away 0.07 s after it was taken.
//!
//! The rules, for every attacker (the leader, AI Sora, Donald, Goofy) and every enemy alike (no table of characters):
//! - every Elden Ring position written for a body is NEVER BELOW THE GROUND UNDER IT: a down-cast from above its head
//!   against the world's collision finds the first floor; feet more than `SINK_TOLERANCE` under it are put on it;
//! - no floor found at all (off the map's collision, the collision not loaded): the body is NOT MOVED DOWN (held at the
//!   height of the last write), and after `NO_GROUND_RELEASE` s of that it is given back to the game where it is;
//! - what the game itself had standing there is the truth: a floor the cast sees above the game's own feet at the take
//!   (water, foliage, a thin prop) is allowed as deep as it was then (`take_bias`), so the clamp never lifts a body the
//!   game put there itself;
//! - the game moving a body we hold by more than `GAME_MOVED` m since our last write (its physics origin shifting, its
//!   own placement) moves every Elden Ring place we keep for it by as much (`game_moved`);
//! - a body ours while Sora's steps stop (a cutscene, a load, a menu that holds the world) is given back after
//!   `UNSTEPPED_RELEASE` writes without a step (`Watchdog`): its gravity, movement and actions are never left off.

use glam::Vec3;

/// A floor is a surface at least this upright (Elden Ring's Y up; kh-react's 0.7 too).
pub const FLOOR_NORMAL_Y: f32 = 0.7;
/// Metres the feet may be under the floor the cast found before they are put on it (kh-react's sweep sphere leaves a
/// standing body 5 cm under its floor; the game's own feet sink a little into soft ground).
pub const SINK_TOLERANCE: f32 = 0.10;
/// Metres above the feet the down-cast starts at least and at most (the body's capsule height, plus `HEAD_CLEARANCE`).
pub const HEAD_MIN: f32 = 1.0;
pub const HEAD_MAX: f32 = 4.0;
pub const HEAD_CLEARANCE: f32 = 0.1;
/// Metres below the feet the down-cast reaches (a body flung off a ledge finds the ground under it; nothing within this:
/// no ground).
pub const CAST_DEPTH: f32 = 60.0;
/// Seconds without any ground under a body before it is given back to the game.
pub const NO_GROUND_RELEASE: f32 = 1.0;
/// Metres the game must have moved a body we hold since our last write for it to be the game's own move (an origin shift
/// is 8 m or more; a push out of an overlap is far less).
pub const GAME_MOVED: f32 = 4.0;
/// Writes (frames) without a step of the reactions before every body we hold is given back.
pub const UNSTEPPED_RELEASE: u32 = 90;
/// A contact at less than this fraction of the cast is the start overlapping something (a branch at the head), not the
/// ground under the body.
const START_OVERLAP: f32 = 1e-4;

/// Where the down-cast starts: above the head of a body `height` m tall standing at `feet`.
pub fn cast_start(feet: Vec3, height: f32) -> Vec3 {
    let h = if height.is_finite() { height } else { 0.0 };
    feet + Vec3::Y * (h.clamp(HEAD_MIN, HEAD_MAX) + HEAD_CLEARANCE)
}

/// One contact of the down-cast: the fraction of the cast, the contact's height, the Y part of its normal.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Seen {
    pub fraction: f32,
    pub y: f32,
    pub normal_y: f32,
}

/// THE GROUND under the body: the first floor the down-cast from `start_y` met (an overlap at the start, a wall, a
/// ceiling's underside and anything above the start are not the ground). None: no floor at all.
pub fn ground_of(seen: impl IntoIterator<Item = Seen>, start_y: f32) -> Option<f32> {
    seen.into_iter()
        .filter(|s| s.fraction.is_finite() && s.y.is_finite() && s.fraction > START_OVERLAP && s.normal_y >= FLOOR_NORMAL_Y && s.y <= start_y)
        .min_by(|a, b| a.fraction.total_cmp(&b.fraction))
        .map(|s| s.y)
}

/// What the cast saw above the game's own feet when the body was taken (m, never negative): that much is the game's own
/// standing, never lifted out of.
pub fn take_bias(feet_y: f32, ground: Option<f32>) -> f32 {
    ground.map_or(0.0, |g| (g - feet_y).max(0.0))
}

/// What the clamp did to a wanted place.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Verdict {
    /// Above the ground (or within the tolerance): as wanted.
    Free,
    /// The feet were `by` m under the ground: put on it.
    Lifted { by: f32 },
    /// No ground under it: not moved down (`by` m of the wanted drop refused).
    Held { by: f32 },
}

/// THE CLAMP: `want` (the reaction's next place), `last` (the place written before), `ground` (`ground_of` at `want`),
/// `bias` (`take_bias` of the take). Returns the place to write.
pub fn clamp(want: Vec3, last: Vec3, ground: Option<f32>, bias: f32) -> (Vec3, Verdict) {
    match ground {
        Some(g) => {
            let floor = g - bias.max(0.0);
            if want.y < floor - SINK_TOLERANCE {
                (Vec3::new(want.x, floor, want.z), Verdict::Lifted { by: floor - want.y })
            } else {
                (want, Verdict::Free)
            }
        }
        None if want.y < last.y => (Vec3::new(want.x, last.y, want.z), Verdict::Held { by: last.y - want.y }),
        None => (want, Verdict::Free),
    }
}

/// The game moved a body we hold since our last write (`we_wrote`) to `game_now` by more than `GAME_MOVED`: the move, by
/// which every Elden Ring place we keep for the body moves. None: it is where we put it (a small push is overruled).
pub fn game_moved(game_now: Vec3, we_wrote: Vec3) -> Option<Vec3> {
    let d = game_now - we_wrote;
    (d.is_finite() && d.length() > GAME_MOVED).then_some(d)
}

/// A body ours this long without ground under it is given back.
pub fn no_ground_too_long(seconds: f32) -> bool {
    seconds > NO_GROUND_RELEASE
}

/// THE RESTORE WATCHDOG: the writes after the game's physics count, a step of the reactions resets; true once when the
/// bodies must be given back (the steps stopped).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Watchdog {
    unstepped: u32,
}

impl Watchdog {
    pub fn stepped(&mut self) {
        self.unstepped = 0;
    }

    /// One write without a step; true at the write that crosses `UNSTEPPED_RELEASE` (once until the next step).
    pub fn wrote(&mut self) -> bool {
        self.unstepped = self.unstepped.saturating_add(1);
        self.unstepped == UNSTEPPED_RELEASE
    }

    pub fn unstepped(&self) -> u32 {
        self.unstepped
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn floor(fraction: f32, y: f32) -> Seen {
        Seen { fraction, y, normal_y: 1.0 }
    }

    #[test]
    fn the_cast_starts_above_the_head_within_its_limits() {
        let feet = Vec3::new(3.0, -2.0, 7.0);
        assert!((cast_start(feet, 1.9).y - (-2.0 + 1.9 + HEAD_CLEARANCE)).abs() < 1e-5);
        assert!((cast_start(feet, 0.0).y - (-2.0 + HEAD_MIN + HEAD_CLEARANCE)).abs() < 1e-5);
        assert!((cast_start(feet, 30.0).y - (-2.0 + HEAD_MAX + HEAD_CLEARANCE)).abs() < 1e-5);
        assert!((cast_start(feet, f32::NAN).y - (-2.0 + HEAD_MIN + HEAD_CLEARANCE)).abs() < 1e-5);
        assert_eq!((cast_start(feet, 1.9).x, cast_start(feet, 1.9).z), (3.0, 7.0));
    }

    #[test]
    fn the_ground_is_the_first_floor_under_the_start() {
        // a wall first, then the floor, then a cave floor further down
        let seen = [Seen { fraction: 0.01, y: 1.5, normal_y: 0.1 }, floor(0.03, 0.4), floor(0.5, -20.0)];
        assert_eq!(ground_of(seen, 2.0), Some(0.4));
        // unordered contacts: the smallest fraction wins
        assert_eq!(ground_of([floor(0.5, -20.0), floor(0.03, 0.4)], 2.0), Some(0.4));
    }

    #[test]
    fn an_overlap_at_the_start_a_ceiling_and_steep_slopes_are_not_the_ground() {
        // a branch the head starts in (fraction 0), a ceiling's underside (normal down), a 60 degree slope
        let seen = [floor(0.0, 1.9), Seen { fraction: 0.02, y: 1.2, normal_y: -1.0 }, Seen { fraction: 0.04, y: 0.8, normal_y: 0.5 }];
        assert_eq!(ground_of(seen, 2.0), None);
        // something reported above the start is not under the body
        assert_eq!(ground_of([floor(0.1, 3.0)], 2.0), None);
        assert_eq!(ground_of([Seen { fraction: f32::NAN, y: 0.0, normal_y: 1.0 }], 2.0), None);
    }

    #[test]
    fn feet_under_the_ground_are_put_on_it() {
        // (the user's c4311: spun 5.5 m into rising ground) the reaction wants the feet 1.6 m under the slope
        let want = Vec3::new(10.0, -1.65, 4.0);
        let (at, v) = clamp(want, Vec3::new(9.0, 0.0, 4.0), Some(-0.05), 0.0);
        assert!((at - Vec3::new(10.0, -0.05, 4.0)).length() < 1e-5);
        assert!(matches!(v, Verdict::Lifted { by } if (by - 1.6).abs() < 1e-4));
    }

    #[test]
    fn a_body_on_or_above_the_ground_is_left_as_it_is() {
        for want_y in [0.0, -0.05, -0.099, 0.5, 30.0] {
            let want = Vec3::new(1.0, want_y, 2.0);
            assert_eq!(clamp(want, Vec3::ZERO, Some(0.0), 0.0), (want, Verdict::Free), "{want_y}");
        }
        // falling toward ground far below: free all the way down to it
        let want = Vec3::new(0.0, -12.0, 0.0);
        assert_eq!(clamp(want, Vec3::new(0.0, -11.5, 0.0), Some(-25.0), 0.0), (want, Verdict::Free));
    }

    #[test]
    fn no_ground_never_moves_a_body_down() {
        let last = Vec3::new(0.0, 4.0, 0.0);
        let (at, v) = clamp(Vec3::new(0.3, 3.2, 0.1), last, None, 0.0);
        assert!((at - Vec3::new(0.3, 4.0, 0.1)).length() < 1e-5);
        assert!(matches!(v, Verdict::Held { by } if (by - 0.8).abs() < 1e-4));
        // rising (a launch) is not held back
        let up = Vec3::new(0.0, 4.5, 0.0);
        assert_eq!(clamp(up, last, None, 0.0), (up, Verdict::Free));
        assert!(!no_ground_too_long(NO_GROUND_RELEASE) && no_ground_too_long(NO_GROUND_RELEASE + 0.02));
    }

    #[test]
    fn what_the_game_had_standing_there_is_never_lifted() {
        // wading: the cast sees a surface 0.5 m above the game's own feet at the take
        let bias = take_bias(-0.5, Some(0.0));
        assert!((bias - 0.5).abs() < 1e-6);
        let stand = Vec3::new(0.0, -0.5, 0.0);
        assert_eq!(clamp(stand, stand, Some(0.0), bias), (stand, Verdict::Free));
        // but not deeper than the game had it
        let (at, v) = clamp(Vec3::new(0.0, -1.5, 0.0), stand, Some(0.0), bias);
        assert!((at.y + 0.5).abs() < 1e-5 && matches!(v, Verdict::Lifted { .. }));
        // the ground under the game's feet (the usual take): no allowance
        assert_eq!(take_bias(0.0, Some(-0.02)), 0.0);
        assert_eq!(take_bias(0.0, None), 0.0);
    }

    #[test]
    fn the_games_own_moves_are_followed_and_small_pushes_are_not() {
        // the physics origin shifted by (32, 0, -16) under a body we hold (the user's log, 313.696 s)
        let wrote = Vec3::new(-30.0, -1.0, 22.0);
        let shift = Vec3::new(32.0, 0.0, -16.0);
        assert_eq!(game_moved(wrote + shift, wrote), Some(shift));
        assert_eq!(game_moved(wrote + Vec3::new(0.3, 0.1, -0.2), wrote), None);
        assert_eq!(game_moved(Vec3::splat(f32::NAN), wrote), None);
    }

    #[test]
    fn bodies_are_given_back_when_the_steps_stop() {
        let mut w = Watchdog::default();
        // steps every few frames (a fast monitor): never
        for _ in 0..1000 {
            assert!(!w.wrote() && !w.wrote() && !w.wrote());
            w.stepped();
        }
        // the steps stop: once, at the crossing write
        let fired: Vec<u32> = (1..=UNSTEPPED_RELEASE * 3).filter(|_| w.wrote()).collect();
        assert_eq!(fired, vec![UNSTEPPED_RELEASE]);
        assert_eq!(w.unstepped(), UNSTEPPED_RELEASE * 3);
        w.stepped();
        assert_eq!(w.unstepped(), 0);
    }
}
