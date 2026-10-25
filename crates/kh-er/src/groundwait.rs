//! (ZETTA 2026-10-23) THE WAIT FOR THE GROUND AFTER THE GAME MOVES ITS PLAYER, for whichever member leads (the user's log
//! kh-er_malenia_zetta2.log: Malenia's phase-change cutscene ended at 346.06 s; the leader took the body in the same frame
//! ("there is ground under him after 0.00 s"); he ran 1 s on what was there, fell (347.3); the physics origin shifted at
//! 348.01 (0, 32, -8); no ground answered his casts; he was put back where he last stood, which had no ground either; 8 s
//! later the wait let him go "all the same" and the fall was his death).
//!
//! THE RULE (generic: a cutscene's end, a warp, fast travel, a load):
//! - after a cutscene or a warp the body is held where the game put it (seated, a level floor at its feet: no fall) for at
//!   least `SETTLE_S`, and `REBASE_S` after the last shift of the physics origin (the game streams the new place in and
//!   moves its origin while it does);
//! - it is given back only once ground has answered the cast under THE GAME'S OWN POSITION for `CONFIRM_S` without a break;
//! - no blind release: without ground it is held on, told every `TELL_S`; only after `GIVE_UP_S` (a place whose floor our
//!   casts can never see: never a stall for good) is it let go, with the level floor kept under it.
//! The game's own actions (a door, a ladder, a pickup) and grabs end as before: given back as soon as ground answers.

/// Seconds the body is held at least after a cutscene's end or a warp.
pub const SETTLE_S: f32 = 2.0;
/// Seconds after the last shift of the physics origin before the body may be given back.
pub const REBASE_S: f32 = 1.0;
/// Seconds ground must answer, without a break, before the body is given back.
pub const CONFIRM_S: f32 = 0.5;
/// (WALLS load fix 2026-10-23, the user: loads must be IMMEDIATE) THE FAST RULE: the game's own body resolved on its ground
/// (its physics module standing / touching solid ground, not falling) AND our cast under the seat answering for this long,
/// this long after the last shift of the origin: given back at once, no minimum hold. Without the game's word the rule
/// above (`SETTLE_S`, `REBASE_S`, `CONFIRM_S`) still decides (a place still streaming in).
pub const FAST_CONFIRM_S: f32 = 0.1;
pub const FAST_REBASE_S: f32 = 0.1;
/// (WALLS load fix) Seconds after a release in which a fall with no ground below at all (a streaming gap the release did not
/// see) takes the leader back into the wait at once instead of falling (sora.rs `follow_phase`).
pub const GUARD_S: f32 = 2.0;

/// (WALLS cutscene fix 2026-10-23, the user: after skipping Malenia's cutscenes "the camera is positioned somewhere random")
/// Whether the end of a wait for the ground puts KH3's camera behind the leader at once (full weight, default framing): after
/// a cutscene (played or skipped), a load / a respawn (`fresh`) or the game's own relocation; not after a door, a ladder or
/// a grab (the camera simply goes on).
pub fn camera_snap_due(after_cutscene: bool, relocated: bool, fresh: bool) -> bool {
    after_cutscene || relocated || fresh
}

/// Seconds between the log's words while it waits.
pub const TELL_S: f32 = 5.0;
/// Seconds without any ground after which the body is let go all the same (the level floor kept under it).
pub const GIVE_UP_S: f32 = 12.0; // (WALLS load fix 2026-10-23) was 30: the user waited 30 s and then fell; the game's placement is now adopted and a buried body put on its floor

/// What the wait says this frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// Held on (no fall).
    Hold,
    /// Ground confirmed: the leader has the body again.
    Release,
    /// No ground for `GIVE_UP_S`: let go all the same.
    GiveUp,
}

/// The wait's clocks.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Wait {
    /// The strict rule (a cutscene's end, a warp, a load): `SETTLE_S`, `REBASE_S`, `CONFIRM_S`; else (an action, a grab)
    /// ground at once is enough.
    pub strict: bool,
    pub since: f32,
    pub since_rebase: f32,
    pub ground_for: f32,
    told: f32,
}

impl Wait {
    pub fn start(strict: bool) -> Wait {
        Wait { strict, since: 0.0, since_rebase: REBASE_S, ground_for: 0.0, told: 0.0 }
    }

    /// One frame: `rebased` (the origin shifted, or the game moved the player, this frame), `ground` (the cast under the
    /// game's own position answered).
    pub fn frame(&mut self, dt: f32, rebased: bool, ground: bool) -> Verdict {
        self.frame_with(dt, rebased, ground, false)
    }

    /// (WALLS load fix) `frame` with the game's own word: `body_ground` = its body stands on solid ground (the fast rule).
    pub fn frame_with(&mut self, dt: f32, rebased: bool, ground: bool, body_ground: bool) -> Verdict {
        self.since += dt;
        if rebased {
            self.since_rebase = 0.0;
            self.ground_for = 0.0;
        } else {
            self.since_rebase += dt;
        }
        self.ground_for = if ground { self.ground_for + dt } else { 0.0 };
        if !self.strict {
            return if ground { Verdict::Release } else if self.since >= GIVE_UP_S { Verdict::GiveUp } else { Verdict::Hold };
        }
        if body_ground && self.since_rebase >= FAST_REBASE_S && self.ground_for >= FAST_CONFIRM_S {
            return Verdict::Release; // (WALLS load fix) the fast rule
        }
        if self.since >= SETTLE_S && self.since_rebase >= REBASE_S && self.ground_for >= CONFIRM_S {
            return Verdict::Release;
        }
        if self.since >= GIVE_UP_S && self.ground_for <= 0.0 {
            return Verdict::GiveUp;
        }
        Verdict::Hold
    }

    /// Time to say it is still waiting (once every `TELL_S`).
    pub fn tell(&mut self) -> bool {
        if self.since - self.told >= TELL_S {
            self.told = self.since;
            return true;
        }
        false
    }
}

/// (WALLS load fix 2026-10-23) What a jump of the body in the physics step (from where we wrote it to where it is after the
/// step) is: None = no jump of the game's (4 m or less: a push); Some(true) = the physics origin shifted (whole multiples of
/// 8 m, game.rs `origin_shift`); Some(false) = THE GAME'S OWN PLACEMENT (a load, a warp), never to be written over.
pub fn game_jump(written: glam::Vec3, now: glam::Vec3) -> Option<bool> {
    let moved = now - written;
    (moved.is_finite() && moved.length() > 4.0).then(|| crate::game::origin_shift(moved))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// (WALLS load fix) THE FAST TRAVEL (the user: "Sora would be stuck in the floor unmoveable for a few seconds"): the game's
    /// body parked under the loading screen (no ground for our cast, the game not standing), the origin shift at 1.00 s, the
    /// place answering and the body standing from 1.05 s: given back within 0.3 s of that, not after the 2 s settle. A
    /// buried body (the game says standing, our cast finds nothing) is never given back on the game's word alone.
    #[test]
    fn groundwait_a_fast_travel_is_given_back_at_once_when_the_game_stands() {
        let dt = 1.0 / 60.0;
        let mut w = Wait::start(true);
        let mut t = 0.0f32;
        let at = loop {
            let rebased = (t - 1.0).abs() < dt * 0.5;
            let ready = t >= 1.05;
            if w.frame_with(dt, rebased, ready, ready) == Verdict::Release {
                break t;
            }
            t += dt;
            assert!(t < 5.0, "held");
        };
        assert!(at >= 1.05 && at - 1.05 <= 0.3, "released at {at}");
        // buried: the game says standing, no cast answers: held (unbury / the slow rule decide)
        let mut w = Wait::start(true);
        for _ in 0..120 {
            assert_eq!(w.frame_with(dt, false, false, true), Verdict::Hold);
        }
        // the game not standing yet (still streaming): the slow rule, as before
        let mut w = Wait::start(true);
        let mut n = 0;
        while w.frame_with(dt, false, true, false) == Verdict::Hold {
            n += 1;
        }
        assert!(n as f32 * dt >= SETTLE_S - 2.0 * dt);
    }

    /// (WALLS cutscene fix) A CUTSCENE SKIPPED MID-WAY with the game's relocation (Malenia's intro / phase two; the leader
    /// Sora, Donald or Goofy: the rule does not depend on who leads): the cutscene's end starts the wait (camera snap due), the
    /// game puts its body elsewhere in the physics step (adopted: its placement, not an origin shift), the place answers and
    /// the body stands at once: given back within 0.3 s, the camera snapped behind the leader. A door or a ladder: no snap.
    #[test]
    fn groundwait_a_skipped_cutscene_with_a_relocation_gives_back_at_once_for_every_leader() {
        use glam::Vec3;
        let dt = 1.0 / 60.0;
        for leader in ["sora", "donald", "goofy"] {
            assert!(camera_snap_due(true, false, false), "{leader}: a cutscene's end snaps the camera");
            // the skip: the body moved by the game in the step (from the seat to the arena's start, not whole 8 m units)
            let seat = Vec3::new(6.125, 3.130, 7.010);
            assert_eq!(game_jump(seat, seat + Vec3::new(-12.37, 0.42, 25.9)), Some(false), "{leader}: the game's placement");
            let mut w = Wait::start(true);
            w.strict = false; // (sora.rs: a placement adopted after the step waits non-strict)
            let mut t = 0.0f32;
            let at = loop {
                if w.frame_with(dt, t < dt, true, true) == Verdict::Release {
                    break t;
                }
                t += dt;
                assert!(t < 1.0, "{leader}: held");
            };
            assert!(at <= 0.3, "{leader}: given back at {at}");
        }
        assert!(!camera_snap_due(false, false, false), "a door / a ladder / a grab: the camera goes on");
        assert!(camera_snap_due(false, true, false) && camera_snap_due(false, false, true), "a relocation, a load");
    }

    /// (WALLS load fix) The user's log kh-er_load_issues.log: the first load's origin shift (44.17 s) and the second load's
    /// placement that our write had overwritten (155.37 s: the body at (0.976, 4.712, 6.001) after the step, written at
    /// (74.137, 52.708, 507.687)).
    #[test]
    fn groundwait_the_games_placement_is_told_from_an_origin_shift() {
        use glam::Vec3;
        let w = Vec3::new(88.976, 60.712, 550.001);
        assert_eq!(game_jump(w, w + Vec3::new(-88.0, -56.0, -544.0)), Some(true));
        assert_eq!(game_jump(Vec3::new(74.137, 52.708, 507.687), Vec3::new(0.976, 4.712, 6.001)), Some(false));
        assert_eq!(game_jump(w, w + Vec3::new(0.3, 0.0, -2.0)), None, "a push is ours to overrule");
        assert!(GIVE_UP_S <= 12.0, "never a half-minute hold");
    }

    /// The user's log: the cutscene over (346.06), ground answering at once (the old place's), the origin shifted at 348.01,
    /// then no ground at all: the body is never given back to fall; held until the ground is there again and confirmed.
    #[test]
    fn groundwait_the_log_after_a_phase_change_cutscene_holds_the_body_until_real_ground() {
        let dt = 1.0 / 60.0;
        let mut w = Wait::start(true);
        let mut t = 0.0;
        let mut released_at = None;
        // 0 .. 1.95 s: ground "answers" (what was there), then the shift, then none for 8 s, then the new floor streams in
        while t < 14.0 {
            let rebased = (t - 1.95f32).abs() < dt * 0.5;
            let ground = t < 1.95 || t > 10.0;
            match w.frame(dt, rebased, ground) {
                Verdict::Release => {
                    released_at = Some(t);
                    break;
                }
                Verdict::GiveUp => panic!("never let go blind within the wait"),
                Verdict::Hold => {}
            }
            t += dt;
        }
        let at = released_at.expect("given back once real ground is there");
        assert!(at > 10.0 + CONFIRM_S - 2.0 * dt, "not taken at 0.00 s, not before the shift, not without ground ({at})");
    }

    #[test]
    fn groundwait_ground_at_once_after_a_warp_still_waits_to_settle_and_a_door_does_not() {
        let dt = 1.0 / 60.0;
        let mut w = Wait::start(true);
        let mut t = 0.0;
        while w.frame(dt, false, true) == Verdict::Hold {
            t += dt;
        }
        assert!((SETTLE_S - dt..SETTLE_S + 0.1).contains(&t), "settles first ({t})");
        // the game's own action (a door): ground at once is enough
        let mut w = Wait::start(false);
        assert_eq!(w.frame(dt, false, true), Verdict::Release);
    }

    #[test]
    fn groundwait_no_ground_ever_is_held_long_then_let_go_never_a_stall_for_good() {
        let dt = 0.1;
        let mut w = Wait::start(true);
        let mut t = 0.0;
        let mut told = 0;
        loop {
            t += dt;
            match w.frame(dt, false, false) {
                Verdict::Hold => {
                    if w.tell() {
                        told += 1;
                    }
                }
                Verdict::GiveUp => break,
                Verdict::Release => panic!("no ground: never released"),
            }
        }
        assert!(t >= GIVE_UP_S - dt && told >= (GIVE_UP_S / TELL_S) as i32); // (WALLS load fix) the cap is 12 s now
    }
}
