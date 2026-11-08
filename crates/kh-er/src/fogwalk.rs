//! (ER-TWEAKS, THE USER 2026-11-06: "when entering fog gate, just make the party leader play their walking animation thru
//! the fog gate instead of using the elden ring animation") THE LEADER WALKS THROUGH THE FOG WITH HIS OWN WALK.
//!
//! WHAT THE GAME DOES (MEASURED, IMPL_ER18, sheet row `own.move.fog`): Y at "Traverse the mist" reaches the game, which plays
//! 60060 (3.00 s) on its hidden body and walks it 1.8 m through the fog wall; its own fog logic (the fog-pass flag, the boss
//! fight's start, the Mimic Tear's wake that reads that flag) runs on that body and that crossing.
//!
//! WHAT THE MOD DID: the leader was seated on the body (`Follow::Action`: carried through, the crossing the game's) and SHOWED
//! THE TARNISHED'S TRAVERSAL POSE retargeted onto him (Sora: `own_over`; a friend leading: `PoseOver`, with his own walk laid
//! over it only once the body moved faster than 25 cm/s for 0.1 s: propfit.rs `WalkThrough`).
//!
//! THE FIX: the seat stays (the game still carries the body through: flag, boss, position untouched), but no pose of the
//! Tarnished's is laid on the leader during a followed action (`own.move.*`: the fog): HIS OWN KH3 WALK is drawn from the
//! first frame, its phase by the distance the body has moved (no foot slide at the fog's 60 cm/s):
//! - Sora (any outfit; the outfit's clips replace his of the same name): KH3's `WalkLoopN` in his body slot (sora.rs
//!   `step_pose_over`, the leap's clip route), its time = (distance / his walk stride) of the clip;
//! - a friend leading (Donald, Goofy, the Mimic, any later one): his own `WalkRunN` (party/leader.rs `walk_pose`, the walk
//!   route that existed), at full weight from the start.
//!
//! Key `fog.walk = 0`: the Tarnished's pose as before.

use glam::Vec3;

/// THE WALK THROUGH A FOLLOWED ACTION (pure): `on` while the action lasts; `distance` (cm, ground plane) the body has moved
/// since it began (the walk's phase), `speed` (cm/s, smoothed), `weight` eased in and out over [`ActionWalk::BLEND_S`].
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ActionWalk {
    pub on: bool,
    pub distance: f32,
    pub speed: f32,
    pub weight: f32,
    /// How many walks have begun (Sora's clip play id: a new walk is a new play).
    pub walks: u32,
    last: Option<Vec3>,
}

impl ActionWalk {
    pub const BLEND_S: f32 = 0.2;
    pub const TAU_S: f32 = 0.15;
    /// A jump larger than this (cm) in one frame is a relocation, not a step.
    pub const JUMP_CM: f32 = 200.0;

    /// One frame: `active` = the leader is carried through a walking action now; `at` = where the body's carry has brought
    /// him (KH3 cm; any frame that moves with the carry); `dt` in seconds.
    pub fn frame(&mut self, active: bool, at: Vec3, dt: f32) -> f32 {
        let moved = match self.last {
            Some(l) if (at - l).length() < Self::JUMP_CM => Vec3::new(at.x - l.x, at.y - l.y, 0.0).length(),
            _ => 0.0,
        };
        self.last = Some(at);
        if active && !self.on {
            self.walks = self.walks.wrapping_add(1);
            self.distance = 0.0;
            self.speed = 0.0;
        }
        self.on = active;
        if dt > 0.0 {
            self.speed += (moved / dt - self.speed) * (1.0 - (-dt / Self::TAU_S).exp());
        }
        if active {
            self.distance += moved;
        }
        let step = if dt > 0.0 { dt / Self::BLEND_S } else { 0.0 };
        self.weight = if active { (self.weight + step).min(1.0) } else { (self.weight - step).max(0.0) };
        self.weight
    }
}

/// Sora's walk stride (cm per cycle of `WalkLoopN`): his walk speed (KH3's run speed x its walk rate, the speed-matched
/// loop's own rule: kh-core states/run.rs) x the cycle's length.
pub fn sora_stride() -> f32 {
    let p = &kh_core::params::SORA_PARAMS;
    p.speed_run * p.speed_walk_rate * kh_core::AnimId::WalkLoopN.data().length
}

/// The time into a walk cycle of `length` s after `distance` cm walked with `stride` cm per cycle (looping).
pub fn clip_time(distance: f32, stride: f32, length: f32) -> f32 {
    if stride <= 0.0 || length <= 0.0 || !distance.is_finite() {
        return 0.0;
    }
    (distance / stride).rem_euclid(1.0) * length
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 1.0 / 60.0;

    /// The fog's 1.8 m in 3 s: the walk is on from the first frame (no Tarnished pose in between), the distance is the
    /// carry's, and it eases out after.
    #[test]
    fn the_fog_walk_is_on_at_once_and_counts_the_carry() {
        let mut w = ActionWalk::default();
        let mut at = Vec3::new(100.0, 50.0, 10.0);
        w.frame(false, at, DT);
        assert!(!w.on && w.weight == 0.0);
        w.frame(true, at, DT);
        assert!(w.on && w.weight > 0.0, "on at the action's first frame, before any move");
        assert_eq!(w.walks, 1);
        for _ in 0..180 {
            at += Vec3::new(1.0, 0.0, 0.05); // 60 cm/s ahead, a little up the floor
            w.frame(true, at, DT);
        }
        assert!((w.distance - 180.0).abs() < 0.5, "the carry's 1.8 m: {}", w.distance);
        assert!((w.speed - 60.0).abs() < 2.0, "60 cm/s: {}", w.speed);
        assert_eq!(w.weight, 1.0);
        let mut frames = 0;
        while w.frame(false, at, DT) > 0.0 {
            frames += 1;
        }
        assert!(!w.on && (11..=13).contains(&frames), "eased out over 0.2 s: {frames}");
        w.frame(true, at, DT);
        assert_eq!((w.walks, w.distance), (2, 0.0), "a new action is a new walk");
    }

    /// An origin shift / relocation is no step.
    #[test]
    fn a_jump_is_no_step() {
        let mut w = ActionWalk::default();
        w.frame(true, Vec3::ZERO, DT);
        w.frame(true, Vec3::new(5000.0, 0.0, 0.0), DT);
        assert_eq!(w.distance, 0.0);
    }

    /// The cycle's time follows the distance: one stride = one cycle; never NaN.
    #[test]
    fn the_clip_time_is_the_distance_over_the_stride() {
        let stride = sora_stride();
        assert!(stride > 100.0 && stride < 250.0, "Sora's walk stride {stride} cm");
        let len = kh_core::AnimId::WalkLoopN.data().length;
        assert!((clip_time(stride * 0.25, stride, len) - 0.25 * len).abs() < 1e-3);
        assert!(clip_time(stride * 1.5, stride, len) < len);
        assert_eq!(clip_time(f32::NAN, stride, len), 0.0);
        assert_eq!(clip_time(10.0, 0.0, len), 0.0);
    }
}
