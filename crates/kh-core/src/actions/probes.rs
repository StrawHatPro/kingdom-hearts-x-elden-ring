//! OWNER: the actions implementer. The world probes of the traversal states (tier 2:
//! docs/sora-actions-spec.md II.4 wall run / pole, II.5 rail / hop point, II.6 ledge / slide
//! slope; each function names the game's probe it is). They ask the host's `World` for sphere
//! sweeps and for its registries (poles, rails, hop points) and component flags (`surface`).
//!
//! `L` = the actor location (`cmc.location`: the movement spec's pawn has its origin at the feet,
//! the capsule centre 91 cm above it). `Rot(v)` = the rotator of a vector (pitch, yaw);
//! `rotate(pitch, yaw, v)` = `FRotator(pitch, yaw, 0).RotateVector(v)` (X forward, Y right, Z up).

use glam::{Vec2, Vec3};

use super::A;
use crate::pawn::{Ctx, Pawn};
use crate::world::{surface, Hit, HopPoint, Pole, World};

pub(crate) fn yaw_of(v: Vec3) -> f32 {
    v.y.atan2(v.x).to_degrees()
}

/// `Rot(n).Pitch`: 0 for a vertical wall, +90 for a floor.
pub(crate) fn pitch_of(n: Vec3) -> f32 {
    n.normalize_or_zero().z.clamp(-1.0, 1.0).asin().to_degrees()
}

pub(crate) fn dir(yaw: f32) -> Vec3 {
    let a = yaw.to_radians();
    Vec3::new(a.cos(), a.sin(), 0.0)
}

pub(crate) fn rotate(pitch: f32, yaw: f32, v: Vec3) -> Vec3 {
    let (sp, cp) = pitch.to_radians().sin_cos();
    let (sy, cy) = yaw.to_radians().sin_cos();
    Vec3::new(cp * cy, cp * sy, sp) * v.x + Vec3::new(-sy, cy, 0.0) * v.y + Vec3::new(-sp * cy, -sp * sy, cp) * v.z
}

fn flagged(w: &dyn World, hit: &Hit, flag: u32) -> bool {
    w.surface_flags(hit.component) & flag != 0
}

/// A sphere sweep with every hit in order (`GeomSweepMulti` with a `PxSphereGeometry`).
fn sphere_all(w: &dyn World, a: Vec3, b: Vec3, r: f32) -> Vec<Hit> {
    w.sweep_capsule_all(a, b, r, r)
}

// ============================================================================ wall run (II.4)

/// What `WallPoint` (0x14063EF00) hands back.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct WallPoint {
    pub component: u32,
    pub point: Vec3,
    pub normal: Vec3,
    /// `end`: the run is over (a ceiling within 200 cm of an up-run).
    pub end: bool,
}

/// `WallValid` (0x1406402E0), the entry classification: the component is flagged
/// `SQEX_bEnableWallRun`, the normal's pitch within 41 degrees of horizontal, the stick
/// (magnitude >= 0.5) within 35 degrees of "into the wall" (class 0, up) or 50 - 130 degrees off
/// (class 1 / 2, side), the path clear and the wall continuing. Returns the class.
pub(crate) fn wall_valid(s: &Pawn, w: &dyn World, normal: Vec3, component: u32) -> Option<u8> {
    if !s.actions.traversal.wall_run_unlocked || s.stick_magnitude < A.wallrun_entry_stick_min {
        return None;
    }
    if w.surface_flags(component) & surface::WALL_RUN == 0 || pitch_of(normal).abs() > A.wallrun_surface_pitch_max {
        return None;
    }
    let d = dir(s.stick_yaw);
    let flat = Vec2::new(normal.x, normal.y);
    let n2 = if flat.length_squared() > 1.0e-8 { flat.normalize() } else { return None };
    let dot = d.x * n2.x + d.y * n2.y;
    let class = if dot < A.wallrun_entry_up_dot {
        0
    } else if dot < -A.wallrun_entry_side_dot || dot > A.wallrun_entry_side_dot {
        return None;
    } else if d.x * n2.y - d.y * n2.x >= 0.0 {
        2
    } else {
        1
    };
    let (pitch, yaw) = (pitch_of(normal), yaw_of(normal));
    let l = s.cmc.location;
    let r = A.wallrun_probe_radius;
    // 1. Clearance: 40 cm off the wall, 200 above the origin: 150 cm of head room (up-run) or
    //    500 cm of free path (side run).
    let s1 = l + rotate(pitch, yaw, Vec3::new(A.wallrun_entry_clear_off, 0.0, A.wallrun_entry_clear_height));
    let e1 = s1 + match class {
        0 => rotate(pitch + 90.0, yaw, Vec3::X * A.wallrun_entry_clear_up),
        1 => rotate(0.0, yaw + 90.0, Vec3::X * A.wallrun_entry_clear_side),
        _ => rotate(0.0, yaw - 90.0, Vec3::X * A.wallrun_entry_clear_side),
    };
    if !sphere_all(w, s1, e1, r).is_empty() {
        return None;
    }
    // 2. The wall continues there.
    let (p3, y3) = (if class == 0 { -pitch } else { 0.0 }, yaw + 180.0);
    let offset = match class {
        0 => Vec3::new(-A.wallrun_entry_clear_off, 0.0, A.wallrun_entry_wall_up),
        1 => Vec3::new(-A.wallrun_entry_clear_off, -A.wallrun_entry_wall_side, A.wallrun_entry_clear_height),
        _ => Vec3::new(-A.wallrun_entry_clear_off, A.wallrun_entry_wall_side, A.wallrun_entry_clear_height),
    };
    let s2 = l + rotate(p3, y3, offset);
    let e2 = s2 + rotate(p3, y3, Vec3::X * A.wallrun_track_len);
    for hit in sphere_all(w, s2, e2, r) {
        if hit.start_penetrating {
            return None;
        }
        if flagged(w, &hit, surface::WALL_RUN) && pitch_of(hit.impact_normal).abs() <= A.wallrun_surface_pitch_max {
            return Some(class);
        }
    }
    None
}

/// `WallPoint` (0x14063EF00), the per-tick wall tracking: A the flagged surface 140 cm along
/// the facing from 40 cm behind the origin; B (up-run) a ceiling within 200 cm ends the run; C a
/// radius-5 sweep from 50 cm above the origin 120 cm along -normal must hit a flagged component.
/// SIMPLIFIED: the corner look-ahead of step B (`roll`, the `WallRunRoll*` clips) is not made.
pub(crate) fn wall_point(s: &Pawn, w: &dyn World, normal: Vec3, class: u8) -> Option<WallPoint> {
    let l = s.cmc.location;
    let (p, y) = (if class == 0 { -pitch_of(normal) } else { 0.0 }, yaw_of(normal) + 180.0);
    let a0 = l + rotate(p, y, Vec3::X * -A.wallrun_track_back);
    let mut found = None;
    for hit in sphere_all(w, a0, a0 + rotate(p, y, Vec3::X * A.wallrun_track_len), A.wallrun_probe_radius) {
        if hit.start_penetrating {
            return None;
        }
        if flagged(w, &hit, surface::WALL_RUN) && pitch_of(hit.impact_normal).abs() <= A.wallrun_surface_pitch_max {
            found = Some(WallPoint { component: hit.component, point: hit.position, normal: hit.impact_normal, end: false });
            break;
        }
    }
    let mut out = found?;
    if class == 0 && w.line_trace(a0, a0 + rotate(p, y, Vec3::Z * A.wallrun_up_block_line)).is_some() {
        out.end = true;
    }
    let c0 = l + Vec3::Z * 50.0;
    let r = A.wallrun_contact_radius;
    for hit in sphere_all(w, c0, c0 - out.normal * A.wallrun_contact_len, r) {
        if hit.start_penetrating {
            return None;
        }
        if flagged(w, &hit, surface::WALL_RUN) {
            return Some(out);
        }
    }
    None
}

/// The wall run's ledge probe (0x14063E620): a sphere of radius 90 from 80 cm ahead and 250 above
/// the origin down 100: the first hit on a component flagged `SQEX_bEnableAutoRun` whose slope is
/// under 50 degrees: the top he runs out onto. Returns (component, point, yaw).
pub(crate) fn wall_top(s: &Pawn, w: &dyn World) -> Option<(u32, Vec3, f32)> {
    let start = s.cmc.location + dir(s.yaw) * 80.0 + Vec3::Z * 250.0;
    for hit in sphere_all(w, start, start - Vec3::Z * 100.0, A.wallrun_ledge_radius) {
        if hit.start_penetrating {
            return None;
        }
        if flagged(w, &hit, surface::AUTO_RUN) && hit.impact_normal.z > 50.0f32.to_radians().cos() {
            return Some((hit.component, hit.position, s.yaw));
        }
    }
    None
}

// ============================================================================ pole (II.4)

/// The pole probe (pawn vt+0x2330 0x1405A7500): no collision query: the registered poles,
/// skipping inactive ones and the one used last; `Test`: the distance from `point` to the pole's
/// segment within `range + m_CheckRadius`; the nearest wins.
pub(crate) fn pole_probe(s: &Pawn, w: &dyn World, point: Vec3, range: f32) -> Option<Pole> {
    let skip = s.actions.traversal.pole_exclude.map(|(id, _)| id);
    let mut best: Option<(f32, Pole)> = None;
    for pole in w.poles() {
        if !pole.active || Some(pole.id) == skip {
            continue;
        }
        let near = closest_on_segment(point, pole.base1, pole.base2);
        let d2 = (point - near).length_squared();
        let reach = range + pole.check_radius;
        if d2 <= reach * reach && best.is_none_or(|(b, _)| d2 < b) {
            best = Some((d2, *pole));
        }
    }
    best.map(|(_, pole)| pole)
}

/// The probe point of the Air Slide / Superslide callers: `L + actorRotation.Rotate((120, 0, 90))`.
pub(crate) fn pole_probe_point(s: &Pawn) -> Vec3 {
    s.cmc.location + rotate(0.0, s.yaw, Vec3::new(A.pole_probe_offset_fwd, 0.0, A.pole_probe_offset_up))
}

pub(crate) fn closest_on_segment(p: Vec3, a: Vec3, b: Vec3) -> Vec3 {
    let ab = b - a;
    let len2 = ab.length_squared();
    if len2 <= 1.0e-8 {
        return a;
    }
    a + ab * ((p - a).dot(ab) / len2).clamp(0.0, 1.0)
}

// ============================================================================ rail (II.5 R1)

/// Which registry a rail is in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RailSource {
    /// `World::rails`: the id is the host's.
    Level,
    /// The Blizzard spell's ice rail (`PawnMagic::rails`, actor tag `BlizzardRail`): the id is
    /// the id of ANY living node of the chain (the nodes die one by one).
    Ice,
}

/// A rail as a polyline with the facts the state reads.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct RailPath {
    pub source: RailSource,
    pub id: u32,
    pub points: Vec<Vec3>,
    /// Per point: `m_SetSlideSpeed` when the node sets one.
    pub speeds: Vec<Option<f32>>,
    pub range_plus: f32,
    pub start_dir: u8,
    pub no_reverse: bool,
    pub no_jump: bool,
    pub narrow: bool,
}

impl RailPath {
    pub fn length(&self) -> f32 {
        self.points.windows(2).map(|p| (p[1] - p[0]).length()).sum()
    }

    /// The point and the forward direction at arc length `d` (clamped), and the segment index.
    pub fn at(&self, d: f32) -> (Vec3, Vec3, usize) {
        let mut left = d.max(0.0);
        let last = self.points.len().saturating_sub(2);
        for (i, p) in self.points.windows(2).enumerate() {
            let seg = p[1] - p[0];
            let len = seg.length();
            if left <= len || i == last {
                let t = if len > 0.0 { (left / len).clamp(0.0, 1.0) } else { 0.0 };
                return (p[0] + seg * t, seg.normalize_or_zero(), i);
            }
            left -= len;
        }
        (self.points.first().copied().unwrap_or(Vec3::ZERO), Vec3::X, 0)
    }

    /// The arc length of the point of the rail nearest to the segment [a, b], and the squared distance.
    pub fn closest(&self, a: Vec3, b: Vec3) -> (f32, f32) {
        let mut best = (0.0, f32::MAX);
        let mut along = 0.0;
        for p in self.points.windows(2) {
            let len = (p[1] - p[0]).length();
            // Sampled: the probe segment is 30 cm long, its two ends and middle are enough.
            for q in [a, (a + b) * 0.5, b] {
                let near = closest_on_segment(q, p[0], p[1]);
                let d2 = (q - near).length_squared();
                if d2 < best.1 {
                    best = (along + (near - p[0]).length(), d2);
                }
            }
            along += len;
        }
        best
    }

    /// The node speed at arc length `d` (forward), interpolated node to node (II.5 R4): a node
    /// that sets no speed counts as the base speed.
    pub fn speed_at(&self, d: f32, base: f32) -> f32 {
        let mut left = d.max(0.0);
        for (i, p) in self.points.windows(2).enumerate() {
            let len = (p[1] - p[0]).length();
            if left <= len || i + 2 == self.points.len() {
                let Some(v0) = self.speeds[i] else { return base };
                let v1 = self.speeds[i + 1].unwrap_or(base);
                return v0 + (v1 - v0) * if len > 0.0 { (left / len).clamp(0.0, 1.0) } else { 0.0 };
            }
            left -= len;
        }
        base
    }
}

/// Every rail there is now: the level's, then the ice rails (chains of at least two nodes).
pub(crate) fn rail_paths(cx: &Ctx) -> Vec<RailPath> {
    let mut out: Vec<RailPath> = cx.world.rails().iter().filter(|r| r.nodes.len() >= 2).map(|r| RailPath { source: RailSource::Level, id: r.id, points: r.nodes.iter().map(|n| n.position).collect(), speeds: r.nodes.iter().map(|n| n.speed).collect(), range_plus: r.nodes.iter().map(|n| n.detect_range_plus).fold(0.0, f32::max), start_dir: r.start_dir, no_reverse: r.no_reverse, no_jump: r.no_jump, narrow: r.narrow }).collect();
    // The ice rails: only nodes with 0.1 s of life or more count (0x1405A7A03); a chain starts at a
    // living node whose predecessor is gone and runs along the links.
    let nodes: Vec<_> = cx.pawn.magic.rails.iter().filter(|n| n.life >= A.rail_blizzard_min_life).collect();
    for first in nodes.iter().filter(|n| n.previous == 0 || !nodes.iter().any(|m| m.id == n.previous)) {
        let mut points = vec![first.position];
        let mut id = first.id;
        while let Some(next) = nodes.iter().find(|m| m.previous == id) {
            points.push(next.position);
            id = next.id;
        }
        if points.len() >= 2 {
            let speeds = vec![None; points.len()];
            // (The id is the LAST node's: the one that lives longest.)
            out.push(RailPath { source: RailSource::Ice, id, points, speeds, range_plus: 0.0, start_dir: 0, no_reverse: false, no_jump: false, narrow: false });
        }
    }
    out
}

/// The rail of `(source, id)` now. An ice rail is found by any of its nodes.
pub(crate) fn rail_path(cx: &Ctx, source: RailSource, id: u32) -> Option<RailPath> {
    let paths = rail_paths(cx);
    match source {
        RailSource::Level => paths.into_iter().find(|p| p.source == source && p.id == id),
        RailSource::Ice => {
            // The chain that holds node `id`: its last node is the path's id; walk the links.
            let nodes = &cx.pawn.magic.rails;
            let mut last = id;
            while let Some(next) = nodes.iter().find(|m| m.previous == last) {
                last = next.id;
            }
            paths.into_iter().find(|p| p.source == source && p.id == last)
        }
    }
}

/// What the rail probe hands back.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct RailHit {
    pub source: RailSource,
    pub id: u32,
    /// Arc length from the first node.
    pub distance: f32,
    pub reversed: bool,
    pub position: Vec3,
    /// The yaw of the travel direction.
    pub yaw: f32,
}

/// The rail probe (pawn vt+0xEA0 0x1405A7740): the probe segment is the actor location to 30 cm
/// above it; a rail within `range + m_DetectRangePlus` of it; the travel direction by
/// `m_StartDir` or by the sign of dot(rail direction, `want`); not within 150 cm of the end in
/// the travel direction (a rail that ends there does not continue); the rail just left is
/// refused until he is more than 200 cm away from it.
pub(crate) fn rail_probe(cx: &mut Ctx, range: f32, want: Vec3) -> Option<RailHit> {
    let start = cx.pawn.cmc.location;
    let end = start + Vec3::Z * A.rail_probe_height;
    let last = cx.pawn.actions.traversal.rail_exclude.map(|(id, _)| id);
    let mut best: Option<(f32, RailHit)> = None;
    let mut near_last = false;
    for path in rail_paths(cx) {
        let (along, d2) = path.closest(start, end);
        let r = range + path.range_plus;
        if Some(path.id) == last {
            near_last |= d2 < A.rail_probe_near_limit_sq;
            continue;
        }
        if d2 > r * r || best.is_some_and(|(b, _)| d2 >= b) {
            continue;
        }
        let (position, forward, _) = path.at(along);
        let reversed = match path.start_dir {
            1 => false,
            2 => true,
            _ => forward.dot(want) < 0.0,
        };
        let length = path.length();
        let ahead = if reversed { along } else { length - along };
        if ahead <= A.rail_end_margin {
            continue;
        }
        let travel = if reversed { -forward } else { forward };
        best = Some((d2, RailHit { source: path.source, id: path.id, distance: along, reversed, position, yaw: yaw_of(travel) }));
    }
    if !near_last {
        // pawn.vt+0x22E8(): the last rail is forgotten.
        cx.pawn.actions.traversal.rail_exclude = None;
    }
    best.map(|(_, hit)| hit)
}

// ============================================================================ hop point (II.5 H1)

/// The hop probe (pawn vt+0x2350 0x1405A6F40): the registered hop points, active, not the one
/// hit last, an end point only with a previous point remembered; `Check`: within `radius +
/// m_CheckRadius` of `pos`; in front of `yaw` or inside its own radius; the higher point wins,
/// then the nearer. Returns the point and the yaw toward it.
/// (Ordered chains, `m_bIsDangle` and `m_bIsHassou` points are not in the seam.)
pub(crate) fn hop_probe(s: &Pawn, w: &dyn World, pos: Vec3, yaw: f32, radius: f32) -> Option<(HopPoint, f32)> {
    let last = s.actions.traversal.last_hop;
    let forward = dir(yaw);
    let mut limit = radius * radius * A.hopping_probe_limit_factor;
    let mut best: Option<(HopPoint, f32)> = None;
    for c in w.hop_points() {
        if !c.active || Some(c.id) == last || (c.end_point && last.is_none()) {
            continue;
        }
        let reach = radius + c.check_radius;
        if (pos - c.position).length_squared() > reach * reach {
            continue;
        }
        if best.is_some_and(|(b, _)| c.position.z < b.position.z) {
            continue;
        }
        let flat = (c.position - pos).truncate();
        let d2 = flat.length_squared();
        if d2 > limit {
            continue;
        }
        let mut to = if d2 > 0.0 { flat.normalize().extend(0.0) } else { forward };
        if d2 <= c.check_radius * c.check_radius || forward.dot(to) >= 0.0 {
            if d2 < A.hopping_probe_dir_free_sq {
                to = forward;
            }
            best = Some((*c, yaw_of(to)));
            limit = d2;
        }
    }
    best
}

// ============================================================================ ledge (II.6 Dangle)

/// What the ledge probe hands back: the component, where his FEET hang and the yaw facing the wall.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Ledge {
    pub component: u32,
    pub position: Vec3,
    pub yaw: f32,
    /// The contact on the ledge's edge.
    pub contact: Vec3,
}

/// The ledge probe (0x140638740 -> 0x140638900) with the Jump tail's arguments (up 250, down 50,
/// radius 100): attempt 0 along the facing, attempts 1 / 2 yawed +45 / -45 degrees: a sphere of
/// radius 50 dropped from `up + 50` to `up - down - 50` above the feet on the axis 50 cm ahead;
/// the first hit on a component flagged `SQEX_bEnableDangle` with a walkable top; the hang point
/// = contact + Rot(yaw) x `m_DangleOffsetPos` (-60, 0, -150); the SPACE test (a radius-50
/// column from the ledge top down to 10 cm below the hanging feet must be free) and the GRIP
/// test (nothing on the ledge between the hang axis and the edge).
/// SIMPLIFIED: attempt 3 (the radius-100 fallback on his own axis) is not made.
pub(crate) fn ledge_probe(s: &Pawn, w: &dyn World, up: f32, down: f32) -> Option<Ledge> {
    let offset = Vec3::new(A.dangle_offset_back, 0.0, A.dangle_offset_down);
    let radius = A.dangle_probe_radius_near;
    let l = s.cmc.location;
    let walkable_z = s.params().floor_walkable_z;
    let mut accepted: Option<(Ledge, f32)> = None;
    for (attempt, turn) in [0.0f32, A.dangle_probe_side_yaw, -A.dangle_probe_side_yaw].into_iter().enumerate() {
        let d = dir(s.yaw + turn);
        // (The sphere starts its radius above `up` and ends its radius below `up - down`.)
        let a = l + d * A.dangle_probe_forward + Vec3::Z * (up + radius);
        let b = a - Vec3::Z * (down + 2.0 * radius);
        let hit = sphere_all(w, a, b, radius).into_iter().find(|h| !h.start_penetrating && flagged(w, h, surface::DANGLE) && h.impact_normal.z >= walkable_z);
        let Some(hit) = hit else { continue };
        let flat = (hit.position - b).truncate();
        if flat.length_squared() < 1.0e-6 {
            continue;
        }
        let toward = flat.normalize().extend(0.0);
        let yaw = yaw_of(toward);
        let position = hit.position + rotate(0.0, yaw, offset);
        // SPACE: (pos.xy, pos.z - O.z) -> (pos.xy, pos.z - 10), radius |O.x| - 10.
        let top = Vec3::new(position.x, position.y, position.z - offset.z);
        if !sphere_all(w, top, Vec3::new(position.x, position.y, position.z - 10.0), A.dangle_space_radius).is_empty() {
            continue;
        }
        // GRIP: radius 5, from the hang axis 5 cm above the ledge toward the contact.
        // (party I1) the start is ImpactPoint.z + 5 (fStack_3c0 = fStack_358 + 5.0, 0x140639B06; the sphere's radius
        // 5 is the shape, not a second lift: sheet row dangle.grip_radius "shape radius 5.0 and +5")
        let grip = Vec3::new(position.x, position.y, hit.position.z + A.dangle_grip_radius);
        let reach = (hit.position - position).truncate().length() - 2.5;
        if reach > 0.0 && !sphere_all(w, grip, grip + toward * reach, A.dangle_grip_radius).is_empty() {
            continue;
        }
        let dot = toward.dot(d);
        let ledge = Ledge { component: hit.component, position, yaw, contact: hit.position };
        match (attempt, accepted) {
            (0, _) => return Some(ledge),
            (_, None) => accepted = Some((ledge, dot)),
            (_, Some((_, first))) if dot >= first => accepted = Some((ledge, dot)),
            _ => {}
        }
    }
    accepted.map(|(ledge, _)| ledge)
}

// ============================================================================ slide slope (II.6)

/// What the slope probes hand back.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub(crate) struct Slope {
    pub ok: bool,
    /// The slope over the next 10 cm, degrees (negative = downhill).
    pub angle: f32,
    pub forced: bool,
    /// No slide floor underfoot.
    pub no_floor: bool,
    /// (fixes pass 1, for the PORT OPTION `Tuning::slide_any_direction`) The yaw of the floor's
    /// fall line, degrees; meaningful when the floor under him is a slope.
    pub fall_yaw: f32,
}

fn slide_floor(w: &dyn World, at: Vec3) -> Option<Hit> {
    sphere_all(w, at + Vec3::Z * 50.0, at - Vec3::Z * 50.0, 5.0).into_iter().find(|h| !h.start_penetrating && flagged(w, h, surface::SLOPE_SLIDE))
}

/// The start probe (0x14063D9C0): a floor component flagged `SQEX_bEnableSlopeSlide` under him
/// whose fall line descends at 25 degrees or more (15 when also `SQEX_bEnableForceSlopeSlide`),
/// he faces downhill (within the cone: dot >= cos 50; forced: cos 80), the same floor 10 cm
/// ahead and 120 cm (forced: 50) clear along it.
/// The start probe; `any_direction` = the PORT OPTION `Tuning::slide_any_direction` (a user
/// decision, not the game's rule): the facing test is dropped and the 10 cm step and the
/// clearance are taken along the fall line instead of along his facing.
pub(crate) fn slope_start_facing(s: &Pawn, w: &dyn World, any_direction: bool) -> Slope {
    let l = s.cmc.location;
    let Some(p1) = slide_floor(w, l) else { return Slope { no_floor: true, ..Slope::default() } };
    let forced = flagged(w, &p1, surface::FORCE_SLOPE_SLIDE);
    let (limit, min_dot, clear) = if forced { (A.slope_start_angle_forced, 80.0f32.to_radians().cos(), 50.0) } else { (A.slope_start_angle, A.slope_facing_dot, A.slope_clearance) };
    let mut out = Slope { forced, ..Slope::default() };
    // The fall line: the normal's rotator with pitch - 90, X axis.
    let n = p1.impact_normal;
    let fall = rotate(pitch_of(n) - 90.0, yaw_of(n), Vec3::X);
    let fall_angle = fall.z.atan2(fall.truncate().length()).to_degrees();
    out.fall_yaw = fall.y.atan2(fall.x).to_degrees();
    if fall_angle > limit || (!any_direction && dir(s.yaw).dot(fall.normalize_or_zero()) < min_dot) {
        return out;
    }
    let ahead = if any_direction { dir(out.fall_yaw) } else { dir(s.yaw) };
    let Some(p2) = slide_floor(w, p1.position + ahead * 10.0) else { return out };
    let step = p2.position - p1.position;
    out.angle = step.z.atan2(step.truncate().length()).to_degrees();
    if out.angle > limit {
        return out;
    }
    let from = p1.position + Vec3::Z * 10.0;
    if !sphere_all(w, from, from + step.normalize_or_zero() * clear, 5.0).is_empty() {
        return out;
    }
    out.ok = true;
    out
}

/// The continuation probe (0x14063D1F0): the two floor sweeps of the start probe; ok while the
/// slope over the next 10 cm is steeper than -15 degrees (a forced floor: any downhill).
pub(crate) fn slope_continue(s: &Pawn, w: &dyn World) -> Slope {
    let l = s.cmc.location;
    let Some(p1) = slide_floor(w, l) else { return Slope { no_floor: true, ..Slope::default() } };
    let forced = flagged(w, &p1, surface::FORCE_SLOPE_SLIDE);
    let mut out = Slope { forced, ..Slope::default() };
    let Some(p2) = slide_floor(w, p1.position + dir(s.yaw) * 10.0) else { return out };
    let step = p2.position - p1.position;
    out.angle = step.z.atan2(step.truncate().length()).to_degrees();
    out.ok = out.angle < if forced { 0.0 } else { A.slope_continue_angle };
    out
}
