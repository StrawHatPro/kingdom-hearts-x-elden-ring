//! What the reaction asks of the host's world: one sweep of the victim's body.

use glam::Vec3;

/// A blocking contact of a sweep.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Contact {
    /// Fraction of the way from start to end at which the body touches: 0 to 1.
    pub time: f32,
    /// Unit normal of the surface touched, pointing out of it (Z up: a floor is (0, 0, 1)).
    pub normal: Vec3,
}

/// The host's collision, as the reaction needs it. Positions are the victim's FOOT point (its
/// origin at ground level, cm, X forward, Y right, Z up); the host knows the body's shape.
pub trait ReactWorld {
    /// Moves the victim's body from `start` to `end` and returns the first blocking contact, or
    /// `None` when the whole move is free. A surface the body merely touches at `start` is
    /// reported (with `time` 0) when the move goes INTO it.
    fn sweep(&self, start: Vec3, end: Vec3) -> Option<Contact>;
}

/// A `kh_core::World` seen by an upright capsule of this size (centre = foot + half height).
pub struct CapsuleIn<'a, W: kh_core::World + ?Sized> {
    pub world: &'a W,
    pub radius: f32,
    pub half_height: f32,
}

impl<W: kh_core::World + ?Sized> ReactWorld for CapsuleIn<'_, W> {
    fn sweep(&self, start: Vec3, end: Vec3) -> Option<Contact> {
        let up = Vec3::Z * self.half_height;
        self.world.sweep_capsule(start + up, end + up, self.radius, self.half_height).map(|h| Contact { time: h.time.clamp(0.0, 1.0), normal: h.impact_normal })
    }
}

/// An infinite floor at `floor_z` and any number of infinite walls (a point on the wall and its
/// unit normal pointing into the room): the world of the unit tests, and enough for a host that
/// has nothing better yet. The body is a point.
#[derive(Clone, Debug, Default)]
pub struct FlatWorld {
    pub floor_z: f32,
    pub walls: Vec<(Vec3, Vec3)>,
}

impl ReactWorld for FlatWorld {
    fn sweep(&self, start: Vec3, end: Vec3) -> Option<Contact> {
        let delta = end - start;
        let mut best: Option<Contact> = None;
        let mut plane = |point: Vec3, normal: Vec3| {
            let (from, to) = ((start - point).dot(normal), (end - point).dot(normal));
            // In front of (or on) the plane and ending behind it: moving into it.
            if from >= -1.0e-3 && to < 0.0 && delta.dot(normal) < 0.0 {
                let time = (from / (from - to)).clamp(0.0, 1.0);
                if best.is_none_or(|b| time < b.time) {
                    best = Some(Contact { time, normal });
                }
            }
        };
        plane(Vec3::new(0.0, 0.0, self.floor_z), Vec3::Z);
        for &(point, normal) in &self.walls {
            plane(point, normal);
        }
        best
    }
}
