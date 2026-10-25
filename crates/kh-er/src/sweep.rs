//! The adapter between what `kh-core` asks of a world (UE 4 / PhysX sweep semantics, upright
//! capsules) and what the host physics offers here: SPHERE casts that report every contact.
//! Pure (KH3 space, cm): the Elden Ring side only supplies `SphereCaster`.
//!
//! What is emulated, and how (each is a departure from real PhysX to keep in mind):
//!
//! - CAPSULE = three spheres. No capsule shape is available yet (the game's `hknpCapsuleShape`
//!   constructor is not located), so an upright capsule of radius r and half height h is swept
//!   as spheres of radius r at its centre and at +-(h - r) on its axis; the earliest contact of
//!   the three wins. Between the spheres the capsule's straight side is under-covered by at most
//!   r - sqrt(r^2 - ((h - r) / 2)^2): 1.8 cm for Sora's body (r 60, h 89). A sphere (h == r) is
//!   one cast, a line (both 0) is a cast of a 0.5 cm sphere.
//! - `Hit::time`: the cast's fraction. `Hit::position`: the contact point reported.
//! - `Hit::normal` (the sweep's): from the contact point to the centre of the sphere that
//!   touched, at the moment of contact. `Hit::impact_normal` (the surface's): the normal the cast
//!   reports; ON AN EDGE the host physics gives the separating direction, not a face normal (item
//!   R4), so (IMPL_ER23A, `Options::edge_faces`) the face is looked up with two thin probes: see `face_of`.
//! - INITIAL OVERLAP: a contact at fraction 0 whose point is closer to the sphere's centre than
//!   r is `start_penetrating`, with `penetration_depth` = r - that distance, along the direction
//!   from the point to the centre. (IMPL_ER23A, `Options::combine_overlaps`) SEVERAL overlaps at once
//!   (a floor and a wall, two plank edges) are answered with ONE translation that clears them all
//!   (what UE's ResolvePenetration arrives at with its second MTD), not with the deepest alone.
//! - "A surface touched at the start is reported only if the move goes into it": a fraction-0
//!   contact whose normal does not oppose the move is dropped (the host physics also reports
//!   nothing for a cast that leaves an overlap, measured in the game).
//! - TWIN BODIES: Elden Ring loads two collision meshes for the same ground (measured: every
//!   contact arrives twice, from two body ids). `sweep_all` keeps one of two contacts that
//!   coincide in time and place, so the "floor component" logic sees one floor.
//! - CRACKS (IMPL_ER23A, `Options::gap`): KH3's floor logic asks "is there walkable ground under his
//!   centre" with LINES and a probe 1 cm thick (FindFloor's line, the perch probe, EdgeFallCheck).
//!   KH3's floors are closed; Elden Ring's plank floors have gaps of a few cm between the boards:
//!   a line through a gap answers "nothing under him" and he is dropped into Falling while his
//!   body rests on the boards (MEASURED in the user's log: 26 s Falling at a fixed height on wood).
//!   So a vertical downward line / thin probe that goes clearly deeper than a sphere of radius
//!   `gap` would (or meets nothing) is answered with what that sphere rests on: see `bridge`.
//! - (IMPL_ER24) THE FLOOR LAYER (`Extra`, `Options::floor_layer`): Elden Ring keeps TWO collision meshes of its map; the
//!   layer Sora's casts use (the camera's family) sees one of them, the layers characters use see the other (MEASURED: 4 cm
//!   higher on open ground, other bodies). Floors that exist only in the characters' mesh (a door's threshold, the ramp
//!   over a flight of stairs) are added: the lowest sphere of a body, lines and probes also take the FLOOR-LIKE contacts
//!   (`floor_like`) of a second cast made with a characters' layer. Walls of that mesh are not taken.
//! - (IMPL_ER24) THE FOOT RULE (`Options::foot`, `foot_support`): Elden Ring's floors are built for its own player's
//!   capsule (radius 40 cm, a round bottom) which cannot go down a hole or a slot narrower than itself. Where KH3's own
//!   floor finding finds nothing (the body's vertical sweep, a vertical line, the perch probe) he is still supported, at the
//!   height of the rim, when a sphere of the foot radius lowered on his axis meets a rim AND such a sphere placed just past
//!   that rim cannot go down either (the other rim, a wall). A ledge (nothing on the other side) stays KH3's.
//! - (IMPL_ER24) LIFTED STARTS (`Options::lift`): vertical lines and the gap / foot spheres start a few cm above where
//!   they were asked to, so that a start a little INSIDE a thin floor (he sank into a board) does not leave it unseen.
//! - (IMPL_ER24) A PUSH OUT OF AN OVERLAP NEVER GOES THROUGH SOMETHING ELSE (`Options::clamp`, `clamp_push`).

use glam::Vec3;
use kh_core::Hit;

/// One contact of a sphere cast, KH3 space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Contact {
    /// Fraction of the displacement at which the sphere touches, 0 ..= 1.
    pub fraction: f32,
    /// The touched point of the surface, cm.
    pub point: Vec3,
    /// Unit normal of the surface there, pointing out of it.
    pub normal: Vec3,
    /// What was touched (0 = unknown).
    pub body: u32,
}

/// (IMPL_ER24) What a cast takes from the host's SECOND collision layer (the characters' mesh of the map).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Extra {
    None,
    /// The contacts a thing can stand on (`floor_like`).
    Floor,
    All,
}

/// The host physics: every contact of a sphere of `radius` moved from `start` by `delta`.
pub trait SphereCaster {
    fn cast(&self, start: Vec3, delta: Vec3, radius: f32, out: &mut Vec<Contact>);
    /// (IMPL_ER24) The same with the second layer's contacts added (a host without one answers as `cast`).
    fn cast_extra(&self, start: Vec3, delta: Vec3, radius: f32, extra: Extra, out: &mut Vec<Contact>) {
        let _ = extra;
        self.cast(start, delta, radius, out);
    }
}

/// (IMPL_ER24) A contact a sphere can REST on: the surface faces up, or the touched point is on the sphere's lower cap
/// (an edge under it: the host gives the separating direction there, which then points up too). A wall's face touched at
/// the sphere's own height is not.
pub fn floor_like(contact: &Contact, start: Vec3, delta: Vec3, radius: f32) -> bool {
    if contact.normal.z >= 0.3 {
        return true;
    }
    let centre = start + delta * contact.fraction;
    contact.normal.z > -0.1 && centre.z - contact.point.z >= 0.3 * radius.max(LINE_RADIUS)
}

/// (IMPL_ER24) The second layer's contacts that `extra` asks for, added to `out`.
pub fn merge_extra(out: &mut Vec<Contact>, second: &[Contact], start: Vec3, delta: Vec3, radius: f32, extra: Extra) {
    match extra {
        Extra::None => {}
        Extra::Floor => out.extend(second.iter().filter(|c| floor_like(c, start, delta, radius)).copied()),
        Extra::All => out.extend_from_slice(second),
    }
}

/// (IMPL_ER23A) What the adapter does beyond the plain emulation. `DEFAULT` is what Elden Ring runs with.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Options {
    /// The surface normal of an EDGE contact is the face's (two thin probes), not the separating direction.
    pub edge_faces: bool,
    /// Several initial overlaps are answered with one translation that clears them all.
    pub combine_overlaps: bool,
    /// Cracks narrower than twice this are closed for vertical lines and thin probes, cm (0 = off).
    pub gap: f32,
    /// (IMPL_ER24) Floors of the host's second collision layer are taken too.
    pub floor_layer: bool,
    /// (IMPL_ER24) The foot rule's radius, cm (0 = off): holes and slots narrower than about twice this hold him.
    pub foot: f32,
    /// (IMPL_ER24) Vertical lines and the gap / foot spheres start this much higher, cm.
    pub lift: f32,
    /// (IMPL_ER24) A push out of an overlap is cut short where it would go into something else.
    pub clamp: bool,
}

impl Options {
    /// The adapter as it was before IMPL_ER23A (keys `w.edge = 0`, `w.mtd = 0`, `w.gap = 0`).
    pub const PLAIN: Options = Options { edge_faces: false, combine_overlaps: false, gap: 0.0, floor_layer: false, foot: 0.0, lift: 0.0, clamp: false };
    /// The adapter of IMPL_ER23A (the build A68CE9EFCE1A; keys `w.floor = 0`, `w.foot = 0`, `w.lift = 0`, `w.clamp = 0`).
    pub const ER23A: Options = Options { edge_faces: true, combine_overlaps: true, gap: 10.0, ..Options::PLAIN };
    /// (`clamp` is OFF: with it a lift rose THROUGH him, the user's log of 4B6C2CFEF72E at 277 - 280 s: the push that
    /// carries him on a rising floor is the push out of that floor. `w.clamp = 1` switches it on.)
    pub const DEFAULT: Options = Options { edge_faces: true, combine_overlaps: true, gap: 10.0, floor_layer: true, foot: 40.0, lift: 6.0, clamp: false };

    fn extra(&self, want: Extra) -> Extra {
        if self.floor_layer { want } else { Extra::None }
    }
}

/// (IMPL_ER24) The foot rule: a rim counts while what stops the fit sphere is no more than this below it, cm.
const FOOT_TILT: f32 = 8.0;
/// (IMPL_ER24) The fit sphere is this much slimmer than the foot: a hole of exactly twice the foot radius lets him fall.
const FOOT_FIT: f32 = 2.0;
/// (IMPL_ER24) He is put level with the rim when his body's own contact leaves him more than this lower, cm.
const FOOT_LEVEL: f32 = 3.0;
/// (IMPL_ER24) The smallest time of a line's hit (`kh-core` reads time 0 as "no hit").
const LINE_MIN_TIME: f32 = 1.0e-4;

/// Radius used for a line.
pub const LINE_RADIUS: f32 = 0.5;
/// A fraction this small is "at the start".
const START_EPS: f32 = 1e-5;
/// Overlaps shallower than this are a touch, not a penetration, cm.
const TOUCH_DEPTH: f32 = 0.05;
/// A cast thinner than this is a probe (no edge lookup of its own; cracks are closed for it), cm.
const PROBE_RADIUS: f32 = 2.0;

/// (IMPL_ER18) The spheres standing for an upright capsule: at its centre and at +-(h - r), and as many between as keep
/// neighbours no farther apart than the radius (a slim capsule's straight side would otherwise be dented between three
/// spheres: 17 cm for r 32, h 89). Sora's KH3 body (r 60, h 89) is three spheres as before.
fn sphere_offsets(radius: f32, half_height: f32) -> ([f32; 6], usize) {
    let straight = half_height - radius;
    let mut out = [0.0; 6];
    if straight <= 0.01 {
        return (out, 1);
    }
    let count = (((2.0 * straight / radius.max(0.01)).ceil() as usize) + 1).clamp(3, 6);
    for (k, o) in out.iter_mut().take(count).enumerate() {
        *o = -straight + 2.0 * straight * k as f32 / (count - 1) as f32;
    }
    (out, count)
}

/// An initial overlap of one sphere: the direction out of it (from the touched point to the sphere's centre), how deep.
#[derive(Clone, Copy, Debug)]
struct Overlap {
    out: Vec3,
    depth: f32,
    contact: Contact,
}

/// A fraction-0 contact that is a real overlap (deeper than a touch).
fn overlap_of(contact: &Contact, sphere_start: Vec3, radius: f32) -> Option<Overlap> {
    if contact.fraction > START_EPS {
        return None;
    }
    let to_centre = sphere_start - contact.point;
    let distance = to_centre.length();
    let depth = (radius - distance).max(0.0);
    (depth > TOUCH_DEPTH).then(|| Overlap { out: if distance > 1e-4 { to_centre / distance } else { contact.normal }, depth, contact: *contact })
}

/// The `Hit` of one contact of one sphere, or `None` when it does not block this move.
fn hit_of(contact: &Contact, sphere_start: Vec3, delta: Vec3, radius: f32) -> Option<Hit> {
    let direction = delta.normalize_or_zero();
    if contact.fraction <= START_EPS {
        let to_centre = sphere_start - contact.point;
        let distance = to_centre.length();
        let normal = if distance > 1e-4 { to_centre / distance } else { contact.normal };
        // Touching or overlapping at the start: only a move INTO the surface is blocked.
        if direction.dot(normal) >= -1e-4 && direction.dot(contact.normal) >= -1e-4 {
            return None;
        }
        let depth = (radius - distance).max(0.0);
        let penetrating = depth > TOUCH_DEPTH;
        return Some(Hit {
            time: 0.0,
            position: contact.point,
            normal,
            impact_normal: contact.normal,
            start_penetrating: penetrating,
            penetration_depth: if penetrating { depth } else { 0.0 },
            component: contact.body,
        });
    }
    let centre = sphere_start + delta * contact.fraction;
    let to_centre = centre - contact.point;
    let normal = if to_centre.length_squared() > 1e-8 { to_centre.normalize() } else { contact.normal };
    Some(Hit {
        time: contact.fraction.clamp(0.0, 1.0),
        position: contact.point,
        normal,
        impact_normal: contact.normal,
        start_penetrating: false,
        penetration_depth: 0.0,
        component: contact.body,
    })
}

/// `a` is the better first hit than `b`: earlier; at the same time the deeper overlap, then the
/// surface most opposed to the move.
fn better(a: &Hit, b: &Hit, direction: Vec3) -> bool {
    if (a.time - b.time).abs() > 1e-6 {
        return a.time < b.time;
    }
    if (a.penetration_depth - b.penetration_depth).abs() > 1e-3 {
        return a.penetration_depth > b.penetration_depth;
    }
    direction.dot(a.impact_normal) < direction.dot(b.impact_normal)
}

/// Every contact of the capsule's spheres, each with the start of the sphere that made it; and the radius cast with.
fn contacts_of(caster: &dyn SphereCaster, start: Vec3, end: Vec3, radius: f32, half_height: f32, opts: &Options) -> (Vec<(Contact, Vec3)>, f32) {
    let delta = end - start;
    let cast_radius = radius.max(LINE_RADIUS);
    let mut all = Vec::new();
    if delta.length_squared() < 1e-10 {
        return (all, cast_radius);
    }
    let (offsets, count) = sphere_offsets(cast_radius, half_height.max(cast_radius));
    let mut contacts = Vec::new();
    for (k, offset) in offsets[..count].iter().enumerate() {
        let sphere_start = start + Vec3::Z * *offset;
        // (IMPL_ER24) the lowest sphere (and a lone sphere) also meets the floors of the second layer
        caster.cast_extra(sphere_start, delta, cast_radius, opts.extra(if k == 0 { Extra::Floor } else { Extra::None }), &mut contacts);
        all.extend(contacts.iter().map(|c| (*c, sphere_start)));
    }
    (all, cast_radius)
}

/// (IMPL_ER23A) ONE TRANSLATION OUT OF SEVERAL OVERLAPS: the shortest push that clears every one of them (each asks for
/// `depth` along its own direction; projections onto the constraints, a few rounds). `None` when they cannot all be
/// cleared (he is between two things closer together than he is wide: the caller keeps the deepest alone).
fn combined_push(overlaps: &[Overlap]) -> Option<Vec3> {
    let mut push = Vec3::ZERO;
    for _ in 0..12 {
        let mut worst = 0.0f32;
        for o in overlaps {
            let short = o.depth - push.dot(o.out);
            if short > 0.0 {
                push += o.out * short;
                worst = worst.max(short);
            }
        }
        if worst < 0.01 {
            break;
        }
    }
    let cleared = overlaps.iter().all(|o| o.depth - push.dot(o.out) < 0.1);
    (cleared && push.length() > TOUCH_DEPTH && push.length() < 200.0).then_some(push)
}

/// The first contact of a thin probe (a line), not counting what it starts on.
fn thin(caster: &dyn SphereCaster, start: Vec3, delta: Vec3, extra: Extra, scratch: &mut Vec<Contact>) -> Option<Contact> {
    caster.cast_extra(start, delta, LINE_RADIUS, extra, scratch);
    scratch.iter().filter(|c| c.fraction > START_EPS).min_by(|a, b| a.fraction.total_cmp(&b.fraction)).copied()
}

/// (IMPL_ER23A) THE FACE UNDER AN EDGE CONTACT. A sphere that touches the edge of a board, a stair nose or a ledge is told
/// the direction from the edge to its centre: a slanted "normal" that `kh-core`'s walkable test (49 degrees) refuses or
/// takes for a ramp. PhysX gives the triangle's own normal there (UE `FindGeomOpposingNormal`; kh-core/src/world.rs R4:
/// "the tread when coming down onto an edge, the riser when moving into it sideways"). Two thin probes find the two
/// faces that meet at the point: straight down just inside it (the tread) and level into it just below it (the riser);
/// the one more opposed to the move is the answer. A contact that is no edge (the tread probe finds the same slanted
/// face = a real slope) and one whose faces are not found keep the reported normal: `None`.
fn face_of(caster: &dyn SphereCaster, point: Vec3, normal: Vec3, direction: Vec3, opts: &Options) -> Option<Vec3> {
    if !(normal.z > 0.05 && normal.z < 0.985) {
        return None;
    }
    let level = Vec3::new(normal.x, normal.y, 0.0).normalize_or_zero();
    if level == Vec3::ZERO {
        return None;
    }
    let mut scratch = Vec::new();
    let tread = thin(caster, point - level + Vec3::Z * 3.0, Vec3::NEG_Z * 6.0, opts.extra(Extra::Floor), &mut scratch).filter(|c| (c.point.z - point.z).abs() < 2.0).map(|c| c.normal);
    if tread.is_some_and(|t| t.dot(normal) > 0.999) {
        return None;
    }
    let riser = thin(caster, point + level * 3.0 - Vec3::Z * 1.5, -level * 6.0, opts.extra(Extra::All), &mut scratch).filter(|c| (c.point - point).truncate().length() < 2.0).map(|c| c.normal);
    match (tread, riser) {
        (Some(t), Some(r)) => Some(if direction.dot(r) < direction.dot(t) - 1e-3 { r } else { t }),
        (Some(t), None) => Some(t),
        (None, Some(r)) if direction.dot(r) < -0.1 => Some(r),
        _ => None,
    }
}

/// `hit` with the face of its edge contact (when it is one and the option is on).
fn with_face(caster: &dyn SphereCaster, mut hit: Hit, direction: Vec3, cast_radius: f32, opts: &Options) -> Hit {
    if opts.edge_faces && cast_radius >= PROBE_RADIUS && !hit.start_penetrating {
        if let Some(face) = face_of(caster, hit.position, hit.impact_normal, direction, opts) {
            hit.impact_normal = face;
        }
    }
    hit
}

/// (IMPL_ER23A) WHAT A SPHERE OF RADIUS `gap` RESTS ON when its lowest point runs down the vertical segment from `top`:
/// (how far below `top` the touched point is, the hit). Only a contact that carries the sphere (facing up) counts.
fn bridge(caster: &dyn SphereCaster, top: Vec3, length: f32, opts: &Options) -> Option<(f32, Hit)> {
    let gap = opts.gap;
    if gap <= LINE_RADIUS || length <= 1.0 {
        return None;
    }
    let mut contacts = Vec::new();
    // (IMPL_ER24) from `lift` higher: a top that is a little inside a thin board still finds the board
    caster.cast_extra(top + Vec3::Z * (gap + opts.lift), Vec3::NEG_Z * (length + opts.lift), gap, opts.extra(Extra::Floor), &mut contacts);
    let c = contacts.iter().filter(|c| c.fraction > START_EPS && c.normal.z > 0.3).min_by(|a, b| a.fraction.total_cmp(&b.fraction)).copied()?;
    let below = (top.z - c.point.z).clamp(0.0, length);
    let face = if opts.edge_faces { face_of(caster, c.point, c.normal, Vec3::NEG_Z, opts).unwrap_or(c.normal) } else { c.normal };
    Some((below, Hit { time: below / length, position: c.point, normal: face, impact_normal: face, start_penetrating: false, penetration_depth: 0.0, component: c.body }))
}

/// The bridged answer wins over the thin cast's (`thin_below`: how far below the top its point is; `None` = it met
/// nothing) when the thin cast went clearly deeper than the sphere: through a crack. (On a slope the two differ by less
/// than `gap`.)
fn crack(thin_below: Option<f32>, bridged: &Option<(f32, Hit)>, opts: &Options) -> bool {
    bridged.as_ref().is_some_and(|(below, _)| thin_below.is_none_or(|t| t > below + 1.5 * opts.gap))
}

fn vertical_down(delta: Vec3) -> bool {
    delta.z < -1.0 && delta.x.abs() + delta.y.abs() < 1.0e-3 * -delta.z
}

/// (IMPL_ER24) WHERE THE FOOT RESTS when KH3's own floor finding finds nothing under `bottom` (the lowest point of the
/// body or probe at the start of a vertical move of `depth` down): (height of the rim, its body). Two casts:
/// A, a sphere of the foot radius lowered on his axis: the FIRST point it meets that can carry it = the rim;
/// B, the fit test: a sphere a little slimmer, placed just past that rim (on the far side of his axis), lowered to the
///    rim's level: when something on ITS far side stops it within `FOOT_TILT` of the rim's height (the other rim of a
///    hole, the far edge of a slot) or it starts inside something there (a wall), Elden Ring's own player could not go
///    down here: he is held at the rim's height. Nothing there = a ledge: `None`, KH3's edge rules decide.
pub fn foot_support(caster: &dyn SphereCaster, bottom: Vec3, depth: f32, opts: &Options) -> Option<(f32, u32)> {
    let r = opts.foot;
    if r <= opts.gap.max(LINE_RADIUS) || depth <= 1.0 {
        return None;
    }
    let lift = opts.lift.max(1.0);
    let mut contacts = Vec::new();
    // (from 14 cm higher still: his body may rest ON the rim, dipped into the hole: the wider sphere must not start inside it)
    let high = lift + 14.0;
    caster.cast_extra(bottom + Vec3::Z * (r + high), Vec3::NEG_Z * (depth + high), r, opts.extra(Extra::Floor), &mut contacts);
    let rim = contacts
        .iter()
        .filter(|c| c.fraction > START_EPS && c.normal.z > 0.3 && c.point.z <= bottom.z + lift + 0.5 && c.point.z >= bottom.z - depth)
        // (the first one touched = the nearest point of the rim: the game reports one contact per triangle, each at its own closest point)
        .min_by(|a, b| a.fraction.total_cmp(&b.fraction))
        .copied()?;
    let off = (rim.point - bottom).truncate();
    let d = off.length();
    if d < 1.0 {
        return Some((rim.point.z, rim.body));
    }
    let o = off / d;
    let axis = rim.point.truncate() - o * r;
    let fit = (r - FOOT_FIT).max(LINE_RADIUS);
    let length = fit + FOOT_TILT + 1.0;
    // (a slight lean away from the rim: something the sphere starts inside of on that side is then moved INTO and reported)
    let delta = Vec3::new(-o.x * 0.5, -o.y * 0.5, -length);
    caster.cast_extra(Vec3::new(axis.x, axis.y, rim.point.z + fit + 1.0), delta, fit, opts.extra(Extra::All), &mut contacts);
    let held = contacts.iter().any(|c| (c.point.truncate() - axis).dot(-o) > 0.0 && c.point.z >= rim.point.z - FOOT_TILT);
    held.then_some((rim.point.z, rim.body))
}

/// The foot rule's answer as a hit of a vertical move of `length` down whose lowest point starts at `bottom`.
fn support_hit(bottom: Vec3, length: f32, (z, body): (f32, u32)) -> Hit {
    Hit { time: ((bottom.z - z) / length).clamp(0.0, 1.0), position: Vec3::new(bottom.x, bottom.y, z), normal: Vec3::Z, impact_normal: Vec3::Z, start_penetrating: false, penetration_depth: 0.0, component: body }
}

/// (IMPL_ER24) THE PUSH OUT OF AN OVERLAP IS CUT SHORT WHERE IT WOULD GO INTO SOMETHING ELSE: each sphere of the body is
/// cast along the push; what it meets on the way (not what it starts in) limits the depth. A push never takes him through
/// a floor, a ceiling or a thin wall. (`kh-core` moves by depth + 0.25.)
fn clamp_push(caster: &dyn SphereCaster, start: Vec3, radius: f32, half_height: f32, hit: &mut Hit, opts: &Options) {
    let want = hit.penetration_depth + 0.25;
    let reach = want + 2.0;
    let (offsets, count) = sphere_offsets(radius, half_height);
    let mut contacts = Vec::new();
    let mut room = f32::MAX;
    for (k, offset) in offsets[..count].iter().enumerate() {
        caster.cast_extra(start + Vec3::Z * *offset, hit.normal * reach, radius, opts.extra(if k == 0 { Extra::Floor } else { Extra::None }), &mut contacts);
        for c in contacts.iter().filter(|c| c.fraction > START_EPS && c.normal.dot(hit.normal) < -0.2) {
            room = room.min(c.fraction * reach - 1.0);
        }
    }
    if room < want {
        hit.penetration_depth = (room - 0.25).clamp(0.0, hit.penetration_depth);
    }
}

/// `World::sweep_capsule`: the first blocking contact.
pub fn sweep_first(caster: &dyn SphereCaster, start: Vec3, end: Vec3, radius: f32, half_height: f32, opts: &Options) -> Option<Hit> {
    let mut hit = first_blocking(caster, start, end, radius, half_height, opts);
    let delta = end - start;
    let cast_radius = radius.max(LINE_RADIUS);
    let half = half_height.max(cast_radius);
    if let Some(h) = hit.as_mut().filter(|h| h.start_penetrating && opts.clamp) {
        clamp_push(caster, start, cast_radius, half, h, opts);
    }
    // (IMPL_ER24) the foot rule: the body's own vertical sweep and the perch probe
    if opts.foot > 0.0 && vertical_down(delta) && hit.is_none_or(|h| !h.start_penetrating) {
        let length = -delta.z;
        let bottom = start - Vec3::Z * half;
        let body = cast_radius >= PROBE_RADIUS && half > cast_radius + 0.5;
        let ask = match hit {
            None => body || cast_radius < PROBE_RADIUS,
            // the body rests on an edge beside his axis: is it the rim of a hole he should stand level over?
            Some(h) => body && h.normal.z < 0.95 && (h.position - start).truncate().length() > 1.0,
        };
        if ask {
            if let Some(support) = foot_support(caster, bottom, length, opts) {
                let own = hit.map(|h| bottom.z - h.time * length);
                if own.is_none_or(|z| z < support.0 - FOOT_LEVEL) {
                    hit = Some(support_hit(bottom, length, support));
                }
            }
        }
    }
    hit
}

fn first_blocking(caster: &dyn SphereCaster, start: Vec3, end: Vec3, radius: f32, half_height: f32, opts: &Options) -> Option<Hit> {
    let delta = end - start;
    let direction = delta.normalize_or_zero();
    let (contacts, cast_radius) = contacts_of(caster, start, end, radius, half_height, opts);
    let mut best: Option<Hit> = None;
    let mut overlaps: Vec<Overlap> = Vec::new();
    for (contact, sphere_start) in &contacts {
        if opts.combine_overlaps {
            if let Some(overlap) = overlap_of(contact, *sphere_start, cast_radius) {
                overlaps.push(overlap);
                continue;
            }
        }
        if let Some(hit) = hit_of(contact, *sphere_start, delta, cast_radius) {
            if best.as_ref().is_none_or(|b| better(&hit, b, direction)) {
                best = Some(hit);
            }
        }
    }
    // (IMPL_ER23A) initial overlaps: one translation out of all of them; a move that goes out along it is free
    if let Some(deepest) = overlaps.iter().copied().max_by(|a, b| a.depth.total_cmp(&b.depth)) {
        match combined_push(&overlaps) {
            Some(push) => {
                let out = push.normalize();
                if direction.dot(out) < -1e-4 {
                    return Some(Hit { time: 0.0, position: deepest.contact.point, normal: out, impact_normal: deepest.contact.normal, start_penetrating: true, penetration_depth: push.length(), component: deepest.contact.body });
                }
            }
            None => {
                // they cannot all be cleared: each as before (the deepest one the move goes into wins)
                for o in &overlaps {
                    if direction.dot(o.out) < -1e-4 || direction.dot(o.contact.normal) < -1e-4 {
                        let hit = Hit { time: 0.0, position: o.contact.point, normal: o.out, impact_normal: o.contact.normal, start_penetrating: true, penetration_depth: o.depth, component: o.contact.body };
                        if best.as_ref().is_none_or(|b| better(&hit, b, direction)) {
                            best = Some(hit);
                        }
                    }
                }
            }
        }
    }
    // (IMPL_ER23A) a thin probe straight down (the perch probe): a crack between boards is closed for it
    if cast_radius < PROBE_RADIUS && vertical_down(delta) && best.is_none_or(|b| !b.start_penetrating) {
        let length = -delta.z;
        let top = start - Vec3::Z * half_height.max(cast_radius);
        let bridged = bridge(caster, top, length, opts);
        if crack(best.map(|b| top.z - b.position.z), &bridged, opts) {
            return bridged.map(|(_, hit)| hit);
        }
    }
    best.map(|hit| with_face(caster, hit, direction, cast_radius, opts))
}

/// `World::sweep_capsule_all`: every component met, each with its first contact, by time.
pub fn sweep_all(caster: &dyn SphereCaster, start: Vec3, end: Vec3, radius: f32, half_height: f32, opts: &Options) -> Vec<Hit> {
    let delta = end - start;
    let direction = delta.normalize_or_zero();
    let (contacts, cast_radius) = contacts_of(caster, start, end, radius, half_height, opts);
    let mut per_body: Vec<Hit> = Vec::new();
    for (contact, sphere_start) in &contacts {
        let Some(hit) = hit_of(contact, *sphere_start, delta, cast_radius) else { continue };
        match per_body.iter_mut().find(|h| h.component == hit.component) {
            Some(existing) => {
                if better(&hit, existing, direction) {
                    *existing = hit;
                }
            }
            None => per_body.push(hit),
        }
    }
    per_body.sort_by(|a, b| a.time.total_cmp(&b.time).then(a.component.cmp(&b.component)));
    // Twin bodies: the same contact from two collision meshes of the same ground.
    let mut out: Vec<Hit> = Vec::with_capacity(per_body.len());
    for hit in per_body {
        let twin = out.iter().any(|h| (h.time - hit.time).abs() < 1e-4 && (h.position - hit.position).length() < 0.5);
        if !twin {
            // (IMPL_ER23A) the first few get their edge's face (the states' ground probe reads the slope of each)
            out.push(if out.len() < 4 { with_face(caster, hit, direction, cast_radius, opts) } else { hit });
        }
    }
    // (IMPL_ER24) the foot rule for the states' ground probe (a sphere straight down): over a hole or a slot that holds him
    // it must not answer "air" (an air attack from where he stands)
    if out.is_empty() && opts.foot > 0.0 && vertical_down(delta) {
        let bottom = start - Vec3::Z * half_height.max(cast_radius);
        if let Some(support) = foot_support(caster, bottom, -delta.z, opts) {
            out.push(support_hit(bottom, -delta.z, support));
        }
    }
    out
}

/// `World::line_trace`: the first surface on the segment. The time is that of the touched point
/// along the segment (the thin sphere's own fraction is early by its radius on slanted surfaces).
pub fn line_first(caster: &dyn SphereCaster, start: Vec3, end: Vec3, opts: &Options) -> Option<Hit> {
    let delta = end - start;
    let length_squared = delta.length_squared();
    if length_squared < 1e-10 {
        return None;
    }
    let mut contacts = Vec::new();
    let down = vertical_down(delta);
    // (IMPL_ER24) a line straight down starts `lift` higher: its start a little inside a thin floor (he sank into a board)
    // would otherwise leave through the board's underside and see nothing under him
    let lift = if down { opts.lift.max(0.0) } else { 0.0 };
    caster.cast_extra(start + Vec3::Z * lift, delta - Vec3::Z * lift, LINE_RADIUS, opts.extra(Extra::Floor), &mut contacts);
    let direction = delta.normalize();
    let line = contacts
        .iter()
        // A line starting on a surface and leaving it does not hit it.
        .filter(|c| !(c.fraction <= START_EPS && direction.dot(c.normal) >= -1e-4))
        .min_by(|a, b| a.fraction.total_cmp(&b.fraction).then(direction.dot(a.normal).total_cmp(&direction.dot(b.normal))))
        .map(|c| {
            let time = ((c.point - start).dot(delta) / length_squared).clamp(0.0, 1.0);
            Hit { time, position: c.point, normal: c.normal, impact_normal: c.normal, start_penetrating: false, penetration_depth: 0.0, component: c.body }
        });
    // (IMPL_ER23A) a line straight down: a crack between boards is closed for it
    if down {
        let length = -delta.z;
        let bridged = bridge(caster, start, length, opts);
        let mut found = if crack(line.map(|l| l.time * length), &bridged, opts) { bridged.map(|(_, hit)| hit) } else { line };
        // (IMPL_ER24) nothing under his centre at all: the foot rule (a hole or a slot narrower than Elden Ring's own player)
        if found.is_none() && opts.foot > 0.0 {
            found = foot_support(caster, start, length, opts).map(|s| support_hit(start, length, s));
        }
        return found.map(|hit| Hit { time: if opts.lift > 0.0 { hit.time.max(LINE_MIN_TIME) } else { hit.time }, ..hit });
    }
    line
}

#[cfg(test)]
#[path = "sweep_tests.rs"]
mod world_tests;

#[cfg(test)]
mod tests {
    use super::*;

    /// (IMPL_ER18) KH3's body stays three spheres; the slim Elden Ring body gets enough to leave no dent wider than its radius.
    #[test]
    fn a_slim_capsule_is_more_spheres() {
        let (o, n) = sphere_offsets(60.0, 89.0);
        assert_eq!((n, o[0], o[1], o[2]), (3, -29.0, 0.0, 29.0));
        let (o, n) = sphere_offsets(32.0, 89.0);
        assert_eq!(n, 5);
        assert!((o[0] + 57.0).abs() < 1e-4 && (o[4] - 57.0).abs() < 1e-4 && o[2].abs() < 1e-4);
        assert!(o[..n].windows(2).all(|w| w[1] - w[0] <= 32.0));
        assert_eq!(sphere_offsets(50.0, 50.0).1, 1);
    }

    /// A world of infinite planes (point, unit normal, body): exact sphere casts.
    struct Planes(Vec<(Vec3, Vec3, u32)>);

    impl SphereCaster for Planes {
        fn cast(&self, start: Vec3, delta: Vec3, radius: f32, out: &mut Vec<Contact>) {
            out.clear();
            for &(point, normal, body) in &self.0 {
                let height = (start - point).dot(normal);
                let approach = -delta.dot(normal);
                if height < radius {
                    // Overlapping at the start. Like the game's physics: reported at fraction 0
                    // unless the cast leaves the surface.
                    if height > -radius && approach > 0.0 {
                        out.push(Contact { fraction: 0.0, point: start - normal * height, normal, body });
                    }
                    continue;
                }
                if approach <= 0.0 {
                    continue;
                }
                let fraction = (height - radius) / approach;
                if fraction <= 1.0 {
                    let centre = start + delta * fraction;
                    out.push(Contact { fraction, point: centre - normal * radius, normal, body });
                }
            }
        }
    }

    const R: f32 = 60.0;
    const H: f32 = 89.0;

    fn floor() -> Planes {
        Planes(vec![(Vec3::ZERO, Vec3::Z, 7)])
    }

    #[test]
    fn a_capsule_lands_on_the_floor_with_its_bottom_sphere() {
        // Centre 200 above the floor, moved 300 down: the bottom (89 below the centre) touches after 111.
        let hit = sweep_first(&floor(), Vec3::new(0.0, 0.0, 200.0), Vec3::new(0.0, 0.0, -100.0), R, H, &Options::DEFAULT).unwrap();
        assert!((hit.time - 111.0 / 300.0).abs() < 1e-5, "{hit:?}");
        assert!(hit.position.length() < 1e-3);
        assert!((hit.normal - Vec3::Z).length() < 1e-4 && (hit.impact_normal - Vec3::Z).length() < 1e-4);
        assert!(!hit.start_penetrating && hit.penetration_depth == 0.0 && hit.component == 7);
    }

    #[test]
    fn a_free_move_and_a_move_along_the_floor_report_nothing() {
        assert!(sweep_first(&floor(), Vec3::new(0.0, 0.0, 200.0), Vec3::new(500.0, 0.0, 200.0), R, H, &Options::DEFAULT).is_none());
        // Resting exactly on the floor, sliding sideways, and lifting off.
        let rest = Vec3::new(0.0, 0.0, H);
        assert!(sweep_first(&floor(), rest, rest + Vec3::X * 100.0, R, H, &Options::DEFAULT).is_none());
        assert!(sweep_first(&floor(), rest, rest + Vec3::Z * 50.0, R, H, &Options::DEFAULT).is_none());
    }

    #[test]
    fn touching_at_the_start_and_moving_in_is_a_hit_at_time_zero() {
        let rest = Vec3::new(0.0, 0.0, H);
        let hit = sweep_first(&floor(), rest, rest - Vec3::Z * 10.0, R, H, &Options::DEFAULT).unwrap();
        assert_eq!(hit.time, 0.0);
        assert!(!hit.start_penetrating, "a touch is not a penetration: {hit:?}");
    }

    #[test]
    fn an_initial_overlap_reports_its_depth() {
        // The bottom sphere's centre is 50 above the floor: 10 deep.
        let start = Vec3::new(0.0, 0.0, 50.0 + (H - R));
        let hit = sweep_first(&floor(), start, start - Vec3::Z * 5.0, R, H, &Options::DEFAULT).unwrap();
        assert!(hit.start_penetrating && hit.time == 0.0);
        assert!((hit.penetration_depth - 10.0).abs() < 1e-3, "{hit:?}");
        assert!((hit.normal - Vec3::Z).length() < 1e-4);
        // Leaving the overlap is free.
        assert!(sweep_first(&floor(), start, start + Vec3::Z * 30.0, R, H, &Options::DEFAULT).is_none());
    }

    #[test]
    fn a_wall_is_met_by_the_capsule_side_with_a_horizontal_normal() {
        let world = Planes(vec![(Vec3::new(300.0, 0.0, 0.0), Vec3::NEG_X, 3)]);
        let start = Vec3::new(0.0, 0.0, 500.0);
        let hit = sweep_first(&world, start, start + Vec3::X * 400.0, R, H, &Options::DEFAULT).unwrap();
        assert!((hit.time - 240.0 / 400.0).abs() < 1e-5);
        assert!((hit.normal - Vec3::NEG_X).length() < 1e-4);
    }

    #[test]
    fn all_hits_are_ordered_and_twins_are_merged() {
        // The floor twice (two bodies, as Elden Ring's two collision meshes) and a second floor lower.
        let world = Planes(vec![(Vec3::ZERO, Vec3::Z, 7), (Vec3::ZERO, Vec3::Z, 8), (Vec3::new(0.0, 0.0, -40.0), Vec3::Z, 9)]);
        let hits = sweep_all(&world, Vec3::new(0.0, 0.0, 100.0), Vec3::new(0.0, 0.0, -100.0), 25.0, 25.0, &Options::DEFAULT);
        assert_eq!(hits.len(), 2, "{hits:?}");
        assert!(hits[0].time < hits[1].time);
        assert_eq!((hits[0].component, hits[1].component), (7, 9));
        assert!((hits[0].time - 75.0 / 200.0).abs() < 1e-5);
    }

    #[test]
    fn a_line_reports_the_point_and_its_time_on_the_segment() {
        let slope_normal = Vec3::new(1.0, 0.0, 1.0).normalize();
        let world = Planes(vec![(Vec3::ZERO, slope_normal, 5)]);
        let (start, end) = (Vec3::new(0.0, 0.0, 100.0), Vec3::new(0.0, 0.0, -100.0));
        let hit = line_first(&world, start, end, &Options::DEFAULT).unwrap();
        // The thin sphere touches the slope beside the line; the time is that of the point's height.
        assert!((hit.impact_normal - slope_normal).length() < 1e-4);
        assert!((hit.time - 0.5).abs() < 0.01, "{hit:?}");
        assert!(line_first(&world, start, start + Vec3::Z * 50.0, &Options::DEFAULT).is_none());
    }
}
