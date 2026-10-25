//! A small analytic world for tests (this crate's and a host's): infinite planes and
//! axis-aligned boxes. Centimetres, Z up, like everything in the core. Not a collision library:
//! it answers the two queries of [`World`] exactly enough to test the simulation against.
//!
//! A plane is a solid half-space: everything behind it (`normal . x < offset`) is solid. A floor
//! at height 0 is `(Vec3::Z, 0.0)`; a wall at x = 500 facing back toward the origin is
//! `(Vec3::NEG_X, -500.0)`. A box is solid between its two corners.

use glam::Vec3;

use crate::world::{surface, Hit, HopPoint, Pole, Rail, World};

/// Contacts closer than this count as touching, cm.
const TOUCH: f32 = 0.01;

#[derive(Clone, Debug, Default)]
pub struct TestWorld {
    /// (unit normal pointing out of the solid, offset along it)
    pub planes: Vec<(Vec3, f32)>,
    /// (low corner, high corner)
    pub boxes: Vec<(Vec3, Vec3)>,
    /// (actions implementer) The traversal flags of components that differ from the mesh default:
    /// (component id, `surface` flags). Component ids: planes 1.., then boxes.
    pub flags: Vec<(u32, u32)>,
    pub poles: Vec<Pole>,
    pub rails: Vec<Rail>,
    pub hops: Vec<HopPoint>,
    /// Water volumes: (low corner, high corner); the surface is the top.
    pub water: Vec<(Vec3, Vec3)>,
}

impl TestWorld {
    /// A flat floor at height 0.
    pub fn floor() -> TestWorld {
        TestWorld { planes: vec![(Vec3::Z, 0.0)], ..TestWorld::default() }
    }

    /// The component id of the box added last (for `with_flags`).
    pub fn last_box(&self) -> u32 {
        (self.planes.len() + self.boxes.len()) as u32
    }

    /// Sets the traversal flags of a component (`surface`; replaces the mesh default).
    pub fn with_flags(mut self, component: u32, flags: u32) -> TestWorld {
        self.flags.retain(|(c, _)| *c != component);
        self.flags.push((component, flags));
        self
    }

    /// Adds flags to the box added last, on top of this world's default (kickable).
    pub fn flagged(self, flags: u32) -> TestWorld {
        let component = self.last_box();
        self.with_flags(component, surface::WALL_KICK | flags)
    }

    pub fn with_plane(mut self, normal: Vec3, offset: f32) -> TestWorld {
        self.planes.push((normal.normalize(), offset));
        self
    }

    /// A plane through `point` with the given normal.
    pub fn with_plane_through(self, point: Vec3, normal: Vec3) -> TestWorld {
        let n = normal.normalize();
        self.with_plane(n, n.dot(point))
    }

    pub fn with_box(mut self, low: Vec3, high: Vec3) -> TestWorld {
        self.boxes.push((low.min(high), low.max(high)));
        self
    }

    /// Distance from an upright capsule to a plane (negative when it overlaps).
    fn plane_clearance(normal: Vec3, offset: f32, centre: Vec3, radius: f32, half_height: f32) -> f32 {
        normal.dot(centre) - offset - radius - (half_height - radius).max(0.0) * normal.z.abs()
    }

    fn sweep_plane(component: u32, normal: Vec3, offset: f32, start: Vec3, end: Vec3, radius: f32, half_height: f32) -> Option<Hit> {
        let (from, to) = (Self::plane_clearance(normal, offset, start, radius, half_height), Self::plane_clearance(normal, offset, end, radius, half_height));
        if to >= from - 1.0e-6 || to > TOUCH {
            return None; // moving along or away from it, or never reaching it
        }
        let time = if from <= TOUCH { 0.0 } else { ((from - TOUCH) / (from - to)).clamp(0.0, 1.0) };
        // The capsule's lowest point along the normal, laid onto the plane.
        let centre = start + (end - start) * time;
        let support = centre - Vec3::Z * ((half_height - radius).max(0.0) * normal.z.signum()) - normal * radius;
        let penetrating = from < -TOUCH;
        Some(Hit { time, position: support - normal * (normal.dot(support) - offset), normal, impact_normal: normal, start_penetrating: penetrating, penetration_depth: if penetrating { -from } else { 0.0 }, component })
    }

    /// Distance from an upright capsule to a box, with the direction from the box to the capsule
    /// at the closest points, the nearest point of the box, and the face a move along `direction`
    /// runs into there.
    fn box_distance(low: Vec3, high: Vec3, centre: Vec3, radius: f32, straight: f32, direction: Vec3) -> (f32, Vec3, Vec3, Vec3) {
        // The point of the capsule's (vertical) axis nearest the box: the middle of the box's height range
        // brought into the axis's (an end of the axis, or a point level with the box).
        let z = ((low.z + high.z) * 0.5).clamp(centre.z - straight, centre.z + straight);
        let point = Vec3::new(centre.x, centre.y, z);
        let closest = point.clamp(low, high);
        let offset = point - closest;
        let length = offset.length();
        if length > 0.0 {
            let normal = offset / length;
            // Of the faces the closest point lies on, the one most opposed to the move.
            let mut face = normal;
            let mut most = 1.0e-3;
            for i in 0..3 {
                for (bound, sign) in [(low[i], -1.0), (high[i], 1.0)] {
                    if (closest[i] - bound).abs() <= 1.0e-3 {
                        let mut n = Vec3::ZERO;
                        n[i] = sign;
                        let against = -direction.dot(n);
                        if against > most {
                            (most, face) = (against, n);
                        }
                    }
                }
            }
            return (length - radius, normal, closest, face);
        }
        // The axis is inside the box: out through the nearest face.
        let mut best = (f32::MAX, Vec3::Z);
        for i in 0..3 {
            for (depth, sign) in [(point[i] - low[i], -1.0), (high[i] - point[i], 1.0)] {
                if depth < best.0 {
                    let mut n = Vec3::ZERO;
                    n[i] = sign;
                    best = (depth, n);
                }
            }
        }
        (-best.0 - radius, best.1, point, best.1)
    }

    /// Conservative advancement of the capsule toward one box (both are convex: the gap along the
    /// closest-point direction closes no faster than the move's component along it).
    fn sweep_box(component: u32, low: Vec3, high: Vec3, start: Vec3, end: Vec3, radius: f32, half_height: f32) -> Option<Hit> {
        let delta = end - start;
        let length = delta.length();
        if !(length > 1.0e-9) {
            return None;
        }
        let direction = delta / length;
        let straight = (half_height - radius).max(0.0);
        let mut travelled = 0.0;
        for i in 0..64 {
            let (distance, normal, point, face) = Self::box_distance(low, high, start + direction * travelled, radius, straight, direction);
            let approach = -direction.dot(normal);
            if distance <= TOUCH {
                if approach <= 1.0e-4 {
                    return None;
                }
                let penetrating = i == 0 && distance < -TOUCH;
                return Some(Hit { time: travelled / length, position: point, normal, impact_normal: face, start_penetrating: penetrating, penetration_depth: if penetrating { -distance } else { 0.0 }, component });
            }
            if approach <= 1.0e-6 {
                return None;
            }
            travelled += (distance - TOUCH * 0.5) / approach;
            if travelled >= length {
                return None;
            }
        }
        None
    }
}

impl World for TestWorld {
    fn sweep_capsule(&self, start: Vec3, end: Vec3, radius: f32, half_height: f32) -> Option<Hit> {
        self.sweep_capsule_all(start, end, radius, half_height).into_iter().next()
    }

    /// One hit per plane or box met, nearest first. Component ids: planes 1.., then boxes.
    fn sweep_capsule_all(&self, start: Vec3, end: Vec3, radius: f32, half_height: f32) -> Vec<Hit> {
        let planes = self.planes.iter().enumerate().filter_map(|(i, &(n, d))| Self::sweep_plane(i as u32 + 1, n, d, start, end, radius, half_height));
        let boxes = self.boxes.iter().enumerate().filter_map(|(i, &(low, high))| Self::sweep_box((self.planes.len() + i) as u32 + 1, low, high, start, end, radius, half_height));
        let mut hits: Vec<Hit> = planes.chain(boxes).collect();
        // Stable: equal times keep the order planes, boxes.
        hits.sort_by(|a, b| a.time.total_cmp(&b.time));
        hits
    }

    fn line_trace(&self, start: Vec3, end: Vec3) -> Option<Hit> {
        // A line is a capsule of no size.
        let mut hit = self.sweep_capsule(start, end, 0.0, 0.0)?;
        hit.normal = hit.impact_normal;
        Some(hit)
    }

    fn surface_flags(&self, component: u32) -> u32 {
        // (A test world is BARE unless flagged: kickable only, so that the stage-1 tests' ledges are
        // not hangable. The engine's default for a mesh is `surface::MESH_DEFAULT`.)
        self.flags.iter().find(|(c, _)| *c == component).map_or(surface::WALL_KICK, |(_, f)| *f)
    }

    fn poles(&self) -> &[Pole] {
        &self.poles
    }

    fn rails(&self) -> &[Rail] {
        &self.rails
    }

    fn hop_points(&self) -> &[HopPoint] {
        &self.hops
    }

    fn water_surface(&self, position: Vec3) -> Option<f32> {
        self.water.iter().find(|(low, high)| position.cmpge(*low).all() && position.cmple(*high).all()).map(|(_, high)| high.z)
    }
}
