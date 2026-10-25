//! (ZETTA 2026-10-23, the user: "Goofy also randomly starts hopping after you do Zettaflare and never stops, fix this
//! behaviour and make him act perfectly") THE AI FRIENDS NEVER LOOP A MOVE. Two parts, both hosts:
//!
//! - THE HAND-BACK'S CLEAN SLATE (`PartyRuntime::zetta_allies_back`): the moment Zettaflare's span ends (its hand-back) every
//!   AI friend's running move of Zettaflare's (the zone exits, the walk to the waiting spot) is stopped and the waiting spots
//!   are cleared, so each is back on its own tree's follow / combat at once.
//! - THE LOOP GUARD ([`HopGuard`]): an AI friend who takes off (a jump, a navlink jump, a hop) [`MAX_HOPS`] times within
//!   [`WINDOW_S`] s is in a loop, whatever started it: his command is stopped, the host puts him back by the leader (the
//!   out-of-view warp), and the line says what he was doing (state, command, place, the leader's), so a cause is in the log.

use std::collections::VecDeque;

use kh_party::MemberId;

/// Take-offs within [`WINDOW_S`] that make a loop.
pub const MAX_HOPS: usize = 4;
/// The window (s).
pub const WINDOW_S: f64 = 6.0;

#[derive(Clone, Debug, Default)]
struct Track {
    air: bool,
    takeoffs: VecDeque<f64>,
}

/// The loop guard's memory of each AI friend.
#[derive(Clone, Debug, Default)]
pub struct HopGuard {
    tracks: Vec<(MemberId, Track)>,
}

impl HopGuard {
    /// One step of friend `id`: `air` = he is in a jump / navlink jump / hop state now. True: a loop (this take-off is the
    /// [`MAX_HOPS`]th within [`WINDOW_S`]); his count starts again.
    pub fn observe(&mut self, id: MemberId, air: bool, now: f64) -> bool {
        let k = match self.tracks.iter().position(|t| t.0 == id) {
            Some(k) => k,
            None => {
                self.tracks.push((id, Track::default()));
                self.tracks.len() - 1
            }
        };
        let t = &mut self.tracks[k].1;
        let took_off = air && !t.air;
        t.air = air;
        while t.takeoffs.front().is_some_and(|f| now - *f > WINDOW_S) {
            t.takeoffs.pop_front();
        }
        if !took_off {
            return false;
        }
        t.takeoffs.push_back(now);
        if t.takeoffs.len() >= MAX_HOPS {
            t.takeoffs.clear();
            return true;
        }
        false
    }

    /// Take-offs of `id` in the window now.
    pub fn count(&self, id: MemberId) -> usize {
        self.tracks.iter().find(|t| t.0 == id).map_or(0, |t| t.1.takeoffs.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// a jump now and then is no loop; four take-offs within six seconds are
    #[test]
    fn hopguard_four_takeoffs_in_six_seconds_are_a_loop() {
        let mut g = HopGuard::default();
        let id = MemberId(2);
        let mut t = 0.0;
        let mut hop = |g: &mut HopGuard, t: &mut f64| {
            let a = g.observe(id, true, *t);
            *t += 0.4;
            let b = g.observe(id, false, *t);
            *t += 0.6;
            a || b
        };
        // one jump every 3 s: never
        for _ in 0..10 {
            assert!(!hop(&mut g, &mut t));
            t += 2.0;
        }
        // a hop every second (after a quiet spell): the fourth trips it
        t += WINDOW_S + 1.0;
        let trips: Vec<bool> = (0..4).map(|_| hop(&mut g, &mut t)).collect();
        assert_eq!(trips, vec![false, false, false, true]);
        assert_eq!(g.count(id), 0, "counted again from nothing");
        // staying in the air is one take-off
        assert!(!g.observe(id, true, t) && !g.observe(id, true, t + 1.0) && g.count(id) == 1);
    }
}
