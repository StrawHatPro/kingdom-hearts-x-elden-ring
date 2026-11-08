//! (F-SHAPES) KH3's CONVEX attack shape (`ETresCollision::CONVEX` 6, DCONVEX 8) as a generic shape of the hit system: a
//! convex hull under the shape's transform, tested EXACTLY against the volumes the hit system tests today (the hurt
//! capsules of every target the host offers, a sphere being a capsule of length 0, and the attack capsules of the clash
//! pass). Nothing here knows an enemy: it is the same run-time test for every Elden Ring enemy (build/sora_tmp/weapons/
//! F-SHAPES/STATE.md).
//!
//! THE TRACE (decompiled C in re/decomp; [A] = asset data):
//! - THE SOURCE (FTresAtkCollShapeSrc, copied into the primitive by UTresAtkCollPrimitive 0x140642CA0): m_ShapeType +0x10
//!   (prim+0x998), m_Mesh +0x18 (CONVEX only: prim->m_CollisionMesh), m_RelativeLocation1 +0x3C (prim+0x8C4),
//!   m_bAbsoluteOffset1 +0x48 (prim+0x985 bit 6), m_RelativeRocation +0x9C (an FRotator, made the quaternion prim+0x8E0 by
//!   FUN_142D8BE00), m_RotAttachType +0xA8 (prim+0x999: TRES_ACRA_NORMAL 0, TRES_ACRA_LOCAL 1), m_Scale +0xAC, m_IncScale
//!   +0xB8, m_IncMaxScale +0xC4 (prim+0x914 / +0x920 / +0x92C). For CONVEX / DCONVEX the size is forced to (1, 1, 1)
//!   (prim+0x8F0) and the size curve is dropped: only the scale makes the shape's size.
//! - THE BODY (0x140641180 case 6): the primitive's own UBodySetup gets a copy of every ConvexElem of m_Mesh's BodySetup
//!   (FUN_14062FDF0 / FUN_14062FB10 / FUN_140514130: VertexData, ElemBox, Transform), then it is cooked (BodySetup
//!   vt+0x250 / +0x238). Every CONVEX of the player's blueprints (Player/p_ex001/*_Wep, p_ex001_Pawn, StylePawn/*) names
//!   `Collision/tres_colltest01` [A]: ONE ConvexElem, 42 vertices: a regular 20-gon of radius 100 at the angles k x 18 deg
//!   on z = -50 and z = +50 and the two cap centres; ElemBox +-100 x +-100 x +-50; Transform identity ([`TRES_COLLTEST01`]).
//! - THE TRANSFORM (Update 0x140647950 case 6 / 8), every tick of an active primitive. T = the socket transform
//!   (FUN_14064BA60: attach NORMAL the weapon mesh's socket or bone, OWNER_ATTACHED_MESH the wielder's mesh, socket None the
//!   owner's root). Unless m_bDisableLocation1Attach once the shape has been placed (prim+0x986 bit 0 and +0x984 bit 2):
//!     location (prim+0x870) = T.translation + (m_bAbsoluteOffset1 ? relLoc x T.scale : T.rotation (relLoc x T.scale));
//!     rotation (prim+0x8B0) = m_RotAttachType LOCAL ? relRot : T.rotation x relRot.
//!   Then, every tick, also for a frozen shape: scale (prim+0x8A0) = FUN_140649BF0(time) x T.scale (not with
//!   m_bIgnoreParentScale, prim+0x984 bit 6). FUN_140649BF0 = hit.rs `Convex::scale_at` (with any m_IncScale component:
//!   each axis max(base + inc x t, 0.01), capped by m_IncMaxScale where that is > 0). 0x140648A80 puts the component
//!   there: SetWorldScale3D (vt+0x3B8) and SetWorldLocationAndRotation (0x144042E60: location and rotation are WORLD
//!   values); case 6 / 8 do not rebuild the body: the scale is the PhysX mesh scale.
//! - THE TEST (Sweep 0x14062A920 case 6 / 8): for each ConvexElem of the primitive's body (vt+0x7C8; elems +0x70, count
//!   +0x78) its PxConvexMesh (FUN_142E53010) as geometry type 4 (eCONVEXMESH) with the mesh scale prim+0x8A0..+0x8A8 and the
//!   pose rotation prim+0x8B0, swept by the scene query 0x144526380 (multi-sweep, channel 0x17 TresAtkCollision with the
//!   response set of hit.rs) from the previous location (prim+0x87C; the current one with m_bDisableSweep) to the current
//!   location (prim+0x870); then Apply 0x140618940 per element, as for every other shape. The rotation and the scale are
//!   not swept.
//!
//! THE PORT: [`Hull`] is the mesh's vertices under (location, rotation, scale). The volume a sweep covers is conv(hull,
//! hull + d) (the query's rotation and scale are fixed), itself a convex point set, so the overlap with a hurt capsule (a
//! segment with a radius) is the exact distance between two convex sets (GJK, [`closest_points`]: exact for polytopes;
//! PhysX's convex queries are GJK-based too) compared with the radius. The time of the first contact along the sweep is
//! found by bisection (the volume swept up to time t only grows with t). A window's shapes are [`Volume`]s: a group of the
//! form weapon's TresAtkColl0 may be several shapes (each its own primitive, one record and hit list per group), each a
//! capsule or a hull.

use glam::{DVec3, Quat, Vec3};

/// `Collision/tres_colltest01`'s ConvexElem 0 VertexData [A: assets_converted/packages/KINGDOM HEARTS III/Content/Collision/
/// tres_colltest01.uasset.json, BodySetup_1 AggGeom.ConvexElems[0]]: the 20-gon of radius 100 at z -50 and +50 (vertex k at
/// -18 x (k + 1) deg) and the two cap centres. The element's Transform is the identity.
pub const TRES_COLLTEST01: [Vec3; 42] = [
    Vec3::new(95.10571, -30.901718, -50.0),
    Vec3::new(80.90176, -58.77856, -50.0),
    Vec3::new(58.77856, -80.90175, -50.0),
    Vec3::new(30.901714, -95.105705, -50.0),
    Vec3::new(0.0, -100.000046, -50.0),
    Vec3::new(-30.901714, -95.1057, -50.0),
    Vec3::new(-58.77855, -80.90173, -50.0),
    Vec3::new(-80.901726, -58.77854, -50.0),
    Vec3::new(-95.105675, -30.901707, -50.0),
    Vec3::new(-100.00002, 3.0616169e-15, -50.0),
    Vec3::new(-95.105675, 30.901707, -50.0),
    Vec3::new(-80.90172, 58.778538, -50.0),
    Vec3::new(-58.778538, 80.90171, -50.0),
    Vec3::new(-30.901707, 95.10567, -50.0),
    Vec3::new(-2.9802322e-6, 100.000015, -50.0),
    Vec3::new(30.901697, 95.10566, -50.0),
    Vec3::new(58.778526, 80.9017, -50.0),
    Vec3::new(80.9017, 58.77853, -50.0),
    Vec3::new(95.10565, 30.901701, -50.0),
    Vec3::new(100.0, 3.0616169e-15, -50.0),
    Vec3::new(95.10571, -30.901718, 50.0),
    Vec3::new(80.90176, -58.77856, 50.0),
    Vec3::new(58.77856, -80.90175, 50.0),
    Vec3::new(30.901714, -95.105705, 50.0),
    Vec3::new(0.0, -100.000046, 50.0),
    Vec3::new(-30.901714, -95.1057, 50.0),
    Vec3::new(-58.77855, -80.90173, 50.0),
    Vec3::new(-80.901726, -58.77854, 50.0),
    Vec3::new(-95.105675, -30.901707, 50.0),
    Vec3::new(-100.00002, -3.0616169e-15, 50.0),
    Vec3::new(-95.105675, 30.901707, 50.0),
    Vec3::new(-80.90172, 58.778538, 50.0),
    Vec3::new(-58.778538, 80.90171, 50.0),
    Vec3::new(-30.901707, 95.10567, 50.0),
    Vec3::new(-2.9802322e-6, 100.000015, 50.0),
    Vec3::new(30.901697, 95.10566, 50.0),
    Vec3::new(58.778526, 80.9017, 50.0),
    Vec3::new(80.9017, 58.77853, 50.0),
    Vec3::new(95.10565, 30.901701, 50.0),
    Vec3::new(100.0, -3.0616169e-15, 50.0),
    Vec3::new(0.0, 3.0616169e-15, -50.0),
    Vec3::new(0.0, -3.0616169e-15, 50.0),
];

/// The hull of a CONVEX mesh by its name (the shape source's m_Mesh); None = a mesh this port has not read.
pub fn mesh(name: &str) -> Option<&'static [Vec3]> {
    match name {
        "tres_colltest01" => Some(&TRES_COLLTEST01),
        _ => None,
    }
}

/// A CONVEX shape placed in the world: the mesh's hull under the primitive's location (prim+0x870), rotation (+0x8B0)
/// and mesh scale (+0x8A0).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hull {
    pub mesh: &'static [Vec3],
    pub centre: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}

impl Hull {
    /// One vertex in the world: centre + rotation (scale x v).
    pub fn vertex(&self, v: Vec3) -> Vec3 {
        self.centre + self.rotation * (self.scale * v)
    }

    /// The vertices in the world.
    pub fn vertices(&self) -> impl Iterator<Item = Vec3> + '_ {
        self.mesh.iter().map(|v| self.vertex(*v))
    }

    /// The hull moved by `d`.
    pub fn moved(&self, d: Vec3) -> Hull {
        Hull { centre: self.centre + d, ..*self }
    }

    /// The radius of the smallest sphere about the centre that holds the hull.
    pub fn reach(&self) -> f32 {
        self.vertices().map(|v| v.distance(self.centre)).fold(0.0, f32::max)
    }

    /// The point set of the volume the hull covers while it moves by `sweep` (the hull and the hull + `sweep`), f64.
    fn swept_points(&self, sweep: Vec3, out: &mut Vec<DVec3>) {
        out.clear();
        out.extend(self.vertices().map(|v| v.as_dvec3()));
        if sweep.length_squared() > 1.0e-12 {
            let d = sweep.as_dvec3();
            for k in 0..out.len() {
                out.push(out[k] + d);
            }
        }
    }

    /// The closest points of the swept volume (the hull moved by up to `sweep`) and the segment `a`..`b`: (on the volume,
    /// on the segment, their distance). Distance 0: they meet.
    pub fn closest_to_segment(&self, sweep: Vec3, a: Vec3, b: Vec3) -> (Vec3, Vec3, f32) {
        let mut points = Vec::with_capacity(self.mesh.len() * 2);
        self.swept_points(sweep, &mut points);
        let (p, q, d) = closest_points_f64(&points, &[a.as_dvec3(), b.as_dvec3()]);
        (p.as_vec3(), q.as_vec3(), d as f32)
    }

    /// The distance from `p` to the hull (0 inside).
    pub fn distance(&self, p: Vec3) -> f32 {
        self.closest_to_segment(Vec3::ZERO, p, p).2
    }
}

/// A contact of a hull with a hurt capsule (or an attack capsule): the time along the sweep (0 .. 1), the contact point on
/// the capsule's surface and the normal from the capsule toward the hull (the conventions of hit.rs `sweep_shape`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Contact {
    pub time: f32,
    pub point: Vec3,
    pub normal: Vec3,
}

/// The first contact of `hull` (its place at the END of the tick), swept from where its centre was (`from`), with the
/// capsule `a`..`b` of radius `r`; None = no contact during the sweep.
pub fn capsule_contact(hull: &Hull, from: Vec3, a: Vec3, b: Vec3, r: f32) -> Option<Contact> {
    let d = hull.centre - from;
    let start = hull.moved(-d);
    let touch = |t: f32| {
        let (on_hull, on_seg, dist) = start.closest_to_segment(d * t, a, b);
        (dist <= r).then_some((on_hull, on_seg, dist))
    };
    let (time, (on_hull, on_seg, dist)) = if let Some(c) = touch(0.0) {
        (0.0, c)
    } else {
        // Not touching at the start: none if the whole sweep misses, else the first time it touches (the swept volume up to t
        // grows with t, so "touches by t" is monotone: bisection).
        let mut hit = touch(1.0)?;
        let (mut lo, mut hi) = (0.0f32, 1.0f32);
        for _ in 0..24 {
            let mid = (lo + hi) * 0.5;
            match touch(mid) {
                Some(c) => (hi, hit) = (mid, c),
                None => lo = mid,
            }
        }
        (hi, hit)
    };
    let normal = if dist > 1.0e-3 {
        (on_hull - on_seg) / dist
    } else {
        // Inside the hull: level, toward its centre at that time (as hit.rs `prism_contact` does inside a prism).
        ((start.centre + d * time - on_seg) * Vec3::new(1.0, 1.0, 0.0)).try_normalize().unwrap_or(Vec3::Z)
    };
    Some(Contact { time, point: on_seg + normal * r, normal })
}

// ------------------------------------------------------------------------------------------------ GJK

#[derive(Clone, Copy)]
struct Vert {
    /// a - b (a point of the Minkowski difference) and its two support points.
    w: DVec3,
    a: DVec3,
    b: DVec3,
}

fn support(points: &[DVec3], d: DVec3) -> DVec3 {
    let mut best = points[0];
    let mut dot = best.dot(d);
    for p in &points[1..] {
        let k = p.dot(d);
        if k > dot {
            (best, dot) = (*p, k);
        }
    }
    best
}

/// The barycentric weights of the point of the affine hull of `y` nearest to the origin; None when the points are
/// degenerate (dependent).
fn affine_weights(y: &[DVec3]) -> Option<[f64; 4]> {
    let n = y.len();
    let mut w = [0.0; 4];
    if n == 1 {
        w[0] = 1.0;
        return Some(w);
    }
    // Minimise |y0 + sum_i mu_i (y_i - y0)|^2: the normal equations M mu = rhs, M_ij = e_i . e_j, rhs_i = -e_i . y0.
    let k = n - 1;
    let e: Vec<DVec3> = y[1..].iter().map(|p| *p - y[0]).collect();
    let mut m = [[0.0f64; 4]; 3];
    let mut scale = 0.0f64;
    for (row, ei) in m.iter_mut().zip(&e) {
        for (cell, ej) in row.iter_mut().zip(&e) {
            *cell = ei.dot(*ej);
        }
        row[k] = -ei.dot(y[0]);
        scale = scale.max(ei.length_squared());
    }
    // Gaussian elimination with partial pivoting.
    for c in 0..k {
        let p = (c..k).max_by(|&a, &b| m[a][c].abs().total_cmp(&m[b][c].abs()))?;
        if m[p][c].abs() <= 1.0e-12 * scale.max(1.0e-30) {
            return None;
        }
        m.swap(c, p);
        for r in 0..k {
            if r != c {
                let (f, pivot) = (m[r][c] / m[c][c], m[c]);
                for (cell, p) in m[r].iter_mut().zip(pivot).skip(c).take(k + 1 - c) {
                    *cell -= f * p;
                }
            }
        }
    }
    let mut sum = 0.0;
    for i in 0..k {
        w[i + 1] = m[i][k] / m[i][i];
        sum += w[i + 1];
    }
    w[0] = 1.0 - sum;
    Some(w)
}

/// The point of the simplex nearest to the origin: the smallest of the faces' affine nearest points that lie in their
/// face (all weights >= 0). Returns (the indices of that face, their weights, the point).
fn nearest_on_simplex(s: &[Vert]) -> (Vec<usize>, Vec<f64>, DVec3) {
    let n = s.len();
    let mut best: Option<(Vec<usize>, Vec<f64>, DVec3, f64)> = None;
    for mask in 1u32..(1 << n) {
        let idx: Vec<usize> = (0..n).filter(|i| mask & (1 << i) != 0).collect();
        let y: Vec<DVec3> = idx.iter().map(|&i| s[i].w).collect();
        let Some(w) = affine_weights(&y) else { continue };
        if w[..idx.len()].iter().any(|x| *x < -1.0e-10) {
            continue;
        }
        let p: DVec3 = y.iter().zip(w).map(|(v, k)| *v * k).sum();
        let d = p.length_squared();
        if best.as_ref().is_none_or(|b| d < b.3) {
            best = Some((idx.clone(), w[..idx.len()].iter().map(|x| x.max(0.0)).collect(), p, d));
        }
    }
    match best {
        Some((i, w, p, _)) => (i, w, p),
        // Unreachable in practice (a single vertex is always a valid face): keep the first vertex.
        None => (vec![0], vec![1.0], s[0].w),
    }
}

/// GJK on f64 point sets: the closest points of conv(a) and conv(b) (on a, on b) and their distance, exactly 0 when
/// the sets meet. Exact for polytopes up to the float tolerance.
fn closest_points_f64(a: &[DVec3], b: &[DVec3]) -> (DVec3, DVec3, f64) {
    let sup = |d: DVec3| {
        let (pa, pb) = (support(a, d), support(b, -d));
        Vert { w: pa - pb, a: pa, b: pb }
    };
    let mut simplex = vec![Vert { w: a[0] - b[0], a: a[0], b: b[0] }];
    let mut weights = vec![1.0];
    let mut v = simplex[0].w;
    let mut meet = false;
    for _ in 0..64 {
        let vv = v.length_squared();
        if vv <= 1.0e-12 {
            meet = true;
            break;
        }
        let s = sup(-v);
        // No support point beyond the current one: v is the nearest point of the difference.
        if vv - v.dot(s.w) <= 1.0e-11 * vv || simplex.iter().any(|x| x.w.distance_squared(s.w) <= 1.0e-18) {
            break;
        }
        let mut grown = simplex.clone();
        grown.push(s);
        let (idx, w, p) = nearest_on_simplex(&grown);
        if p.length_squared() >= vv {
            // No progress (numerical noise): v is as near as it gets.
            break;
        }
        simplex = idx.iter().map(|&i| grown[i]).collect();
        weights = w;
        v = p;
        if simplex.len() == 4 {
            // The origin lies in a tetrahedron of the difference: the sets meet.
            meet = true;
            break;
        }
    }
    let pa: DVec3 = simplex.iter().zip(&weights).map(|(s, w)| s.a * *w).sum();
    let pb: DVec3 = simplex.iter().zip(&weights).map(|(s, w)| s.b * *w).sum();
    (pa, pb, if meet { 0.0 } else { v.length() })
}

/// The closest points of two convex point sets (GJK): (on `a`, on `b`).
pub fn closest_points(a: &[Vec3], b: &[Vec3]) -> (Vec3, Vec3) {
    let a: Vec<DVec3> = a.iter().map(|p| p.as_dvec3()).collect();
    let b: Vec<DVec3> = b.iter().map(|p| p.as_dvec3()).collect();
    let (p, q, _) = closest_points_f64(&a, &b);
    (p.as_vec3(), q.as_vec3())
}

// ------------------------------------------------------------------------------------------------ a window's shapes

/// One shape of a hit window or shot: a capsule (a sphere when `a == b`) or a CONVEX hull.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Volume {
    Capsule { a: Vec3, b: Vec3, radius: f32 },
    Hull(Hull),
}

impl Volume {
    /// The shape's middle (a capsule's centre, the hull's location).
    pub fn centre(&self) -> Vec3 {
        match self {
            Volume::Capsule { a, b, .. } => (*a + *b) * 0.5,
            Volume::Hull(h) => h.centre,
        }
    }

    /// m_bDisableLocation1Attach (Update 0x140647950): once placed, the location and the rotation stay where they were;
    /// the size (a capsule's radius and half length, a hull's scale) still grows.
    pub fn kept_at(&self, placed: &Volume) -> Volume {
        match (*self, *placed) {
            (Volume::Capsule { a, b, radius }, Volume::Capsule { a: was_a, b: was_b, .. }) => {
                let (centre, half) = ((was_a + was_b) * 0.5, (b - a).length() * 0.5);
                let axis = (was_b - was_a).try_normalize().unwrap_or(Vec3::ZERO);
                Volume::Capsule { a: centre - axis * half, b: centre + axis * half, radius }
            }
            (Volume::Hull(now), Volume::Hull(then)) => Volume::Hull(Hull { centre: then.centre, rotation: then.rotation, ..now }),
            (now, _) => now,
        }
    }
}

/// A per-frame track of one shape (the `vol` rows of the form sheets, written by the form generators with the FK of the
/// shape's socket): 10 numbers per frame, frame f at f x length / (frames - 1); a CAPSULE [ax, ay, az, bx, by, bz, r, 0, 0,
/// 0], a HULL [x, y, z, qx, qy, qz, qw, sx, sy, sz] (the location prim+0x870, the rotation prim+0x8B0, the scale prim+0x8A0
/// with its growth of the window). Positions are in the frame of the owner (Sora's actor space, his yaw 0; the Clock's root);
/// the rotation too, unless `world_rotation` (m_RotAttachType LOCAL: the shape's rotation is relRot in the world).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VolTrack {
    /// The hull's mesh; None = a capsule.
    pub mesh: Option<&'static [Vec3]>,
    pub world_rotation: bool,
    pub frames: usize,
    pub length: f32,
    pub values: &'static [f32],
}

/// The numbers per frame of a [`VolTrack`].
pub const VOL_STRIDE: usize = 10;

impl VolTrack {
    /// A track from a `vol` row: flags [CAPSULE | HULL, the mesh (HULL), actor | world, ...], code = frames, its length and values.
    pub fn of_row(flags: &[&str], frames: i32, length: f32, values: &'static [f32]) -> Option<VolTrack> {
        let frames = usize::try_from(frames).ok().filter(|f| *f > 0)?;
        if values.len() < frames * VOL_STRIDE {
            return None;
        }
        let mesh = match flags.first().copied() {
            Some("HULL") => Some(mesh(flags.get(1).copied().unwrap_or("-"))?),
            Some("CAPSULE") => None,
            _ => return None,
        };
        Some(VolTrack { mesh, world_rotation: flags.get(2) == Some(&"world"), frames, length, values })
    }

    /// The shape at a clip time in the owner's frame (linear between frames; the rotation normalised-linear).
    pub fn local_at(&self, time: f32) -> Volume {
        let last = self.frames - 1;
        let f = if self.length > 0.0 && last > 0 { (time / self.length).clamp(0.0, 1.0) * last as f32 } else { 0.0 };
        let (i, t) = ((f.floor() as usize).min(last), f - f.floor());
        let j = (i + 1).min(last);
        let v = |frame: usize, k: usize| self.values[frame * VOL_STRIDE + k];
        let at = |k: usize| v(i, k) + (v(j, k) - v(i, k)) * t;
        match self.mesh {
            None => Volume::Capsule { a: Vec3::new(at(0), at(1), at(2)), b: Vec3::new(at(3), at(4), at(5)), radius: at(6) },
            Some(mesh) => {
                let q0 = Quat::from_xyzw(v(i, 3), v(i, 4), v(i, 5), v(i, 6));
                let mut q1 = Quat::from_xyzw(v(j, 3), v(j, 4), v(j, 5), v(j, 6));
                if q0.dot(q1) < 0.0 {
                    q1 = -q1;
                }
                let rotation = (q0 * (1.0 - t) + q1 * t).normalize();
                Volume::Hull(Hull { mesh, centre: Vec3::new(at(0), at(1), at(2)), rotation, scale: Vec3::new(at(7), at(8), at(9)) })
            }
        }
    }

    /// The shape at a clip time for an owner at `location` with `yaw` (degrees about Z).
    pub fn placed(&self, time: f32, location: Vec3, yaw: f32) -> Volume {
        let turn = Quat::from_rotation_z(yaw.to_radians());
        let world = |p: Vec3| location + turn * p;
        match self.local_at(time) {
            Volume::Capsule { a, b, radius } => Volume::Capsule { a: world(a), b: world(b), radius },
            Volume::Hull(h) => Volume::Hull(Hull { centre: world(h.centre), rotation: if self.world_rotation { h.rotation } else { turn * h.rotation }, ..h }),
        }
    }
}

/// A shot whose shape is a tracked hull (combat/hit.rs `Shot::hull`): the track (in the shot actor's frame: its location
/// and yaw) and the clip time of the track at the shot's spawn; `last` = the hull's centre of the previous tick (the sweep's
/// start).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HullShot {
    pub track: VolTrack,
    pub t0: f32,
    pub last: Option<Vec3>,
}

/// Where every window made of [`Volume`]s left its shapes at the previous tick (by `Window::key`): the sweep's start and
/// m_bDisableLocation1Attach's place.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Placed(pub Vec<((u16, u16), Vec<Volume>)>);

impl Placed {
    pub fn take(&mut self, key: (u16, u16)) -> Option<Vec<Volume>> {
        let k = self.0.iter().position(|(w, _)| *w == key)?;
        Some(self.0.swap_remove(k).1)
    }

    pub fn put(&mut self, key: (u16, u16), shapes: Vec<Volume>) {
        self.0.push((key, shapes));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// tres_colltest01 x (sx, sy, sz) upright at `c`.
    fn prism(c: Vec3, s: Vec3) -> Hull {
        Hull { mesh: &TRES_COLLTEST01, centre: c, rotation: Quat::IDENTITY, scale: s }
    }

    #[test]
    fn the_mesh_is_the_20_gon_prism_of_radius_100_and_height_100() {
        assert_eq!(TRES_COLLTEST01.len(), 42);
        for v in &TRES_COLLTEST01[..40] {
            assert!((v.truncate().length() - 100.0).abs() < 0.01 && v.z.abs() == 50.0, "{v}");
            let a = v.y.atan2(v.x).to_degrees();
            assert!(((a / 18.0) - (a / 18.0).round()).abs() < 1.0e-3, "a vertex every 18 deg: {a}");
        }
        assert_eq!(mesh("tres_colltest01").map(<[Vec3]>::len), Some(42));
        assert!(mesh("tres_colltest02").is_none());
    }

    #[test]
    fn gjk_is_exact_against_the_closed_forms() {
        let h = prism(Vec3::ZERO, Vec3::new(8.0, 8.0, 10.0));
        // A vertex (0 deg) reaches 800; the middle of an edge (9 deg) 800 cos 9 deg; the caps +-500.
        assert!(h.distance(Vec3::new(799.0, 0.0, 0.0)) == 0.0);
        assert!((h.distance(Vec3::new(810.0, 0.0, 0.0)) - 10.0).abs() < 1.0e-3);
        let mid = 800.0 * 9f32.to_radians().cos();
        let d = Vec3::new(9f32.to_radians().cos(), 9f32.to_radians().sin(), 0.0);
        assert!(h.distance(d * (mid - 0.5)) == 0.0);
        assert!((h.distance(d * (mid + 20.0)) - 20.0).abs() < 1.0e-2, "{}", h.distance(d * (mid + 20.0)));
        assert!((h.distance(Vec3::new(0.0, 0.0, 530.0)) - 30.0).abs() < 1.0e-3);
        assert!((h.distance(Vec3::new(0.0, 0.0, -600.0)) - 100.0).abs() < 1.0e-3);
        // A corner: beyond the rim and above the cap at once.
        assert!((h.distance(Vec3::new(830.0, 0.0, 540.0)) - 50.0).abs() < 1.0e-2);
        // Two point sets: unit cubes 3 apart along x.
        let cube = |o: Vec3| (0..8).map(move |i| o + Vec3::new((i & 1) as f32, ((i >> 1) & 1) as f32, ((i >> 2) & 1) as f32)).collect::<Vec<_>>();
        let (p, q) = closest_points(&cube(Vec3::ZERO), &cube(Vec3::new(4.0, 0.5, 0.0)));
        assert!((p.x - 1.0).abs() < 1.0e-5 && (q.x - 4.0).abs() < 1.0e-5 && (q - p).length() - 3.0 < 1.0e-5, "{p} {q}");
    }

    #[test]
    fn inside_outside_and_edge_against_a_hurt_capsule() {
        // SW_Finish2 grown: radius 800, half height 500, centre at 100 up.
        let h = prism(Vec3::new(0.0, 0.0, 100.0), Vec3::new(8.0, 8.0, 10.0));
        let body = |x: f32, y: f32, z: f32| capsule_contact(&h, h.centre, Vec3::new(x, y, z), Vec3::new(x, y, z + 100.0), 40.0);
        // Inside: the contact normal points to the hull's centre.
        let c = body(0.0, -100.0, 0.0).expect("inside");
        assert!(c.time == 0.0 && c.normal.y > 0.9, "{c:?}");
        // Outside beside it: 800 + 40 is the edge at a vertex direction.
        assert!(body(839.0, 0.0, 0.0).is_some() && body(841.0, 0.0, 0.0).is_none());
        // Over the cap (top 600): a body whose feet are 30 above it touches with its 40 radius, 50 above does not.
        assert!(body(0.0, 0.0, 630.0).is_some() && body(0.0, 0.0, 650.0).is_none());
        // The old upright-capsule / sphere stand-in (a sphere of 800 at the centre) reached 900 up: KH3's prism does not.
        assert!(body(0.0, 0.0, 760.0).is_none());
        // An edge of the 20-gon: between two vertices the hull is 800 cos 9 deg = 790.15 out.
        let d = Vec3::new(9f32.to_radians().cos(), 9f32.to_radians().sin(), 0.0) * (790.15 + 39.0);
        assert!(capsule_contact(&h, h.centre, d, d + Vec3::Z * 100.0, 40.0).is_some());
        let d = d + Vec3::new(9f32.to_radians().cos(), 9f32.to_radians().sin(), 0.0) * 2.0;
        assert!(capsule_contact(&h, h.centre, d, d + Vec3::Z * 100.0, 40.0).is_none());
    }

    #[test]
    fn a_turned_hull_lies_on_its_side() {
        // The Clock's FinishDR: tres_colltest01 x (10, 10, 15) turned by roll 90 (its axis along the world's y): radius 1000 in
        // x / z, half length 750 in y.
        let h = Hull { rotation: Quat::from_rotation_x(90f32.to_radians()), ..prism(Vec3::ZERO, Vec3::new(10.0, 10.0, 15.0)) };
        assert!(h.distance(Vec3::new(0.0, 0.0, 990.0)) == 0.0, "1000 up");
        assert!(h.distance(Vec3::new(0.0, 740.0, 0.0)) == 0.0 && (h.distance(Vec3::new(0.0, 760.0, 0.0)) - 10.0).abs() < 1.0e-2, "750 along y");
        assert!((h.distance(Vec3::new(0.0, 0.0, 1010.0)) - 10.0).abs() < 1.0e-2);
    }

    #[test]
    fn a_growing_hull_and_the_sweep() {
        // ChangeSW-like growth by the track's scale: 200 at first, 800 later.
        let small = prism(Vec3::ZERO, Vec3::new(2.0, 2.0, 12.0));
        let big = prism(Vec3::ZERO, Vec3::new(8.0, 8.0, 12.0));
        let at = |h: &Hull| capsule_contact(h, h.centre, Vec3::new(500.0, 0.0, 0.0), Vec3::new(500.0, 0.0, 100.0), 40.0);
        assert!(at(&small).is_none() && at(&big).is_some(), "growth reaches the body");
        // Swept: a hull that moved past a body this tick touches it on the way (time between 0 and 1), not at either end.
        let moved = prism(Vec3::new(1000.0, 0.0, 0.0), Vec3::new(1.0, 1.0, 1.0));
        let c = capsule_contact(&moved, Vec3::new(-1000.0, 0.0, 0.0), Vec3::new(0.0, 300.0, -10.0), Vec3::new(0.0, 300.0, 10.0), 250.0).expect("on the way");
        // It first touches when its edge (about 100 out) is 250 from the body: centre at x = -sqrt(350^2 - 300^2) about -180.
        assert!(c.time > 0.3 && c.time < 0.5, "{c:?}");
        assert!(capsule_contact(&moved, moved.centre, Vec3::new(0.0, 300.0, -10.0), Vec3::new(0.0, 300.0, 10.0), 250.0).is_none(), "not without the sweep");
    }

    #[test]
    fn a_track_frame_is_placed_in_the_owner_frame() {
        static V: [f32; 20] = [100.0, 0.0, 50.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0, 200.0, 0.0, 50.0, 0.0, 0.0, 0.0, 1.0, 3.0, 3.0, 1.0];
        let t = VolTrack::of_row(&["HULL", "tres_colltest01", "actor"], 2, 1.0, &V).expect("a hull track");
        let Volume::Hull(h) = t.placed(0.5, Vec3::new(10.0, 0.0, 0.0), 90.0) else { panic!("a hull") };
        assert!((h.centre - Vec3::new(10.0, 150.0, 50.0)).length() < 1.0e-3 && (h.scale.x - 2.0).abs() < 1.0e-5, "{h:?}");
        assert!((h.rotation * Vec3::X - Vec3::Y).length() < 1.0e-5, "turned with the owner's yaw");
        let w = VolTrack::of_row(&["HULL", "tres_colltest01", "world"], 2, 1.0, &V).expect("a hull track");
        let Volume::Hull(h) = w.placed(0.5, Vec3::ZERO, 90.0) else { panic!("a hull") };
        assert!((h.rotation * Vec3::X - Vec3::X).length() < 1.0e-5, "m_RotAttachType LOCAL: the world rotation stays relRot");
        assert!(VolTrack::of_row(&["HULL", "unknown_mesh", "actor"], 2, 1.0, &V).is_none());
        let kept = Volume::Hull(h).kept_at(&Volume::Hull(Hull { centre: Vec3::ONE, ..h }));
        assert_eq!(kept.centre(), Vec3::ONE);
    }
}
