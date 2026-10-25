//! (FOL2) THE FOLLOW SPOTS THE EQS GIVES THE FRIENDS, MADE WALKABLE AND CORRIDOR-AWARE (OUR DESIGN; the user, 2026-10-20:
//! "they will start quickly running between two points, and in narrow hallways they will stand very close together when
//! there is still room for them to spread out").
//!
//! 1. REACHABLE SPOTS. KH3 projects its follow / random-walk items onto the navmesh, so a spot behind a wall never exists.
//!    The Elden Ring host has no navmesh: its nav answers "clear" when the step's cast budget is spent, and spots behind walls
//!    won. The best items of the follow-type queries are now checked by `Nav::reachable_strict` (never optimistic): the
//!    first that the leader or the friend can walk to straight wins; items found unreachable are dropped. Hosts that do not
//!    know (`None`, the test level's straight nav) keep KH3's pick.
//! 2. CORRIDORS. Rays from the leader's feet (`Nav::ray`, every 15 degrees over half a turn, both ways) measure the narrowest
//!    width round him. Narrower than CORRIDOR_ENTER (left above CORRIDOR_EXIT) with room along it: a hallway. There the
//!    follow query's spot is replaced by the member's slot in a file behind the leader along the hallway, staggered across it
//!    as the width allows: the k-th follower (roster order, any leader, any member) at SLOT_FIRST + k x SLOT_STEP behind,
//!    alternately left / right of the centre line by at most SLOT_STAGGER, never nearer a wall than WALL_MARGIN past his
//!    capsule. Open ground keeps KH3's formation untouched.

use glam::Vec3;

use crate::api::ActorRef;
use crate::bt::Ctx;
use crate::eqs::{Item, Querier};

/// THE SWITCH (on by default, as kh-party `locomotion::FOL2_ON`): off = KH3's spots untouched (before FOL2). The host sets it every frame.
pub static ON: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

/// Narrower than this round the leader: a hallway (cm).
pub const CORRIDOR_ENTER: f32 = 520.0;
/// A hallway ends when wider than this (hysteresis, cm).
pub const CORRIDOR_EXIT: f32 = 640.0;
/// The probes' reach (cm) and height above the feet (knee, as the trail nav's straight test).
pub const PROBE_LEN: f32 = 900.0;
pub const PROBE_UP: f32 = 45.0;
/// Along the hallway there must be at least this much room (both ways together, x the width).
pub const ALONG_RATIO: f32 = 1.6;
/// The first follower's slot behind the leader, and the step to the next (cm): at least two Goofy capsules apart.
pub const SLOT_FIRST: f32 = 230.0;
pub const SLOT_STEP: f32 = 170.0;
/// The stagger across the hallway (cm, at most) and the room kept to a wall past the capsule (cm).
pub const SLOT_STAGGER: f32 = 70.0;
pub const WALL_MARGIN: f32 = 25.0;
/// The leader standing: a follower within (nearer, farther) cm of his slot's distance keeps his own distance.
pub const SLOT_BAND: (f32, f32) = (200.0, 160.0);
/// The band's near end is never nearer the leader than SLOT_MIN_LEAD + SLOT_MIN_MATE x the follower's place (cm, centres).
pub const SLOT_MIN_LEAD: f32 = 130.0;
pub const SLOT_MIN_MATE: f32 = 110.0;
/// The corridor is measured again when the leader moved this far (cm) or after this long (s).
pub const REMEASURE_CM: f32 = 60.0;
pub const REMEASURE_S: f64 = 0.25;
/// The follow-type queries (by pack name) whose winner must be reachable.
pub const REACH_QUERIES: &[&str] = &["n_Common_FollowPlayerQuery", "n_Common_FollowPlayerWithSwimQuery", "n_Common_FollowPrecedeQuery", "n_Common_RandomWalkAroundPlayer"];
/// The queries whose spot becomes the hallway slot.
pub const SLOT_QUERIES: &[&str] = &["n_Common_FollowPlayerQuery", "n_Common_FollowPlayerWithSwimQuery"];
/// Items checked for reachability per query run (the nav's strict casts are a few lines each).
pub const REACH_CHECKS: usize = 10;
/// A member's random-walk spot is kept for the others' queries this long (s): longer than the walk (KH3: 6 - 8 s).
pub const ACTION_DEST_LIFE: f64 = 12.0;

/// The hallway round the leader as last measured.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Corridor {
    /// Where (the leader's feet) and when it was measured.
    pub at: Vec3,
    pub time: f64,
    /// A hallway now (with the hysteresis).
    pub narrow: bool,
    /// The hallway's direction (unit, horizontal; sign arbitrary) and across it (unit).
    pub axis: Vec3,
    pub across: Vec3,
    /// The width across at the leader, and the centre line's point beside him.
    pub width: f32,
    pub centre: Vec3,
    /// Free room along the axis from the leader: (+axis, -axis), cm.
    pub room: (f32, f32),
}

fn yaw_dir(yaw: f32) -> Vec3 {
    let (s, c) = yaw.to_radians().sin_cos();
    Vec3::new(c, s, 0.0)
}

/// Measures the hallway round `p` (feet) by rays; `was` = the last answer (its hysteresis). None = the nav cannot cast.
pub fn measure(c: &Ctx, p: Vec3, was: bool, now: f64) -> Option<Corridor> {
    let nav = c.world.nav();
    let o = p + Vec3::Z * PROBE_UP;
    let dist = |d: Vec3| -> Option<f32> { nav.ray(o, o + d * PROBE_LEN).map(|h| h.min(PROBE_LEN)) };
    // the host must be able to cast at all: one probe answered Some(..) or the world is open (all None = unknown)
    let mut any = false;
    let mut best: Option<(f32, f32, f32, Vec3)> = None; // (width, d+, d-, across)
    for k in 0..12 {
        let dir = yaw_dir(15.0 * k as f32);
        let a = dist(dir);
        let b = dist(-dir);
        any |= a.is_some() || b.is_some();
        let (a, b) = (a.unwrap_or(PROBE_LEN), b.unwrap_or(PROBE_LEN));
        if best.is_none_or(|x| a + b < x.0) {
            best = Some((a + b, a, b, dir));
        }
    }
    let (width, a, b, across) = best?;
    if !any {
        return Some(Corridor { at: p, time: now, ..Corridor::default() });
    }
    let axis = Vec3::new(-across.y, across.x, 0.0);
    let (fa, fb) = (dist(axis).unwrap_or(PROBE_LEN), dist(-axis).unwrap_or(PROBE_LEN));
    let limit = if was { CORRIDOR_EXIT } else { CORRIDOR_ENTER };
    let narrow = width < limit && fa + fb >= ALONG_RATIO * width;
    let centre = p + across * ((a - b) * 0.5);
    Some(Corridor { at: p, time: now, narrow, axis, across, width, centre, room: (fa, fb) })
}

/// The hallway round the leader, measured again when stale (kept in `AiShared::corridor`).
pub fn corridor(c: &Ctx) -> Option<Corridor> {
    let p = c.world.player().floor_position;
    let now = c.st.time;
    let mut slot = c.shared.corridor.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(k) = *slot {
        let moved = Vec3::new(k.at.x - p.x, k.at.y - p.y, 0.0).length();
        if moved < REMEASURE_CM && (now - k.time).abs() < REMEASURE_S {
            return Some(k);
        }
    }
    let was = slot.is_some_and(|k| k.narrow);
    let k = measure(c, p, was, now)?;
    *slot = Some(k);
    Some(k)
}

/// The querier's place among the followers (roster order: the members' ids, the leader left out), and how many follow.
pub fn rank(c: &Ctx, q: &Querier) -> (usize, usize) {
    let p = c.world.player();
    let mut al = Vec::new();
    c.world.allies(&mut al);
    let me = match q.actor {
        ActorRef::Member(m) => m,
        _ => q.member,
    };
    let mut ids: Vec<u8> = al
        .iter()
        .filter(|a| a.alive && a.actor != p.actor.actor)
        .filter_map(|a| match a.actor {
            ActorRef::Member(m) => Some(m),
            _ => None,
        })
        .collect();
    if !ids.contains(&me) {
        ids.push(me);
    }
    ids.sort_unstable();
    ids.dedup();
    (ids.iter().position(|&m| m == me).unwrap_or(0), ids.len())
}

/// The querier's slot in the hallway behind the leader, None on open ground (KH3's spot stands).
pub fn hallway_slot(c: &Ctx, q: &Querier) -> Option<Vec3> {
    let k = corridor(c)?;
    if !k.narrow {
        return None;
    }
    let p = c.world.player();
    // behind = against the way he goes (his average motion; standing: his facing)
    let a = c.st.player_motion.avg_dir;
    let ahead = if Vec3::new(a.x, a.y, 0.0).length() > 0.3 { Vec3::new(a.x, a.y, 0.0).normalize() } else { yaw_dir(p.actor.yaw) };
    let (mut back, mut room) = if ahead.dot(k.axis) >= 0.0 { (-k.axis, k.room.1) } else { (k.axis, k.room.0) };
    let (n, _) = rank(c, q);
    let want = SLOT_FIRST + SLOT_STEP * n as f32;
    // no room behind (a dead end): the file goes ahead of him
    if room < want + q.radius + WALL_MARGIN {
        let other = if back == -k.axis { k.room.0 } else { k.room.1 };
        if other > room {
            back = -back;
            room = other;
        }
    }
    // the leader standing: where he already is along the file counts (within the band), so a friend who ran a little past
    // or stopped a little short is not sent back and forth for it
    let still = Vec3::new(p.actor.velocity.x, p.actor.velocity.y, 0.0).length() < 50.0;
    let cur = (q.pos - k.centre).dot(back);
    let near = (want - SLOT_BAND.0).max(SLOT_MIN_LEAD + SLOT_MIN_MATE * n as f32);
    let want = if still { cur.clamp(near.min(want), want + SLOT_BAND.1) } else { want };
    let along = want.min((room - q.radius - WALL_MARGIN).max(0.0));
    if along < 120.0 {
        return None;
    }
    let half = k.width * 0.5 - q.radius - WALL_MARGIN;
    // a canonical "left" of the file: across turned to the back direction's left
    let left = if back.x * k.across.y - back.y * k.across.x > 0.0 { k.across } else { -k.across };
    // the side: on entering, by the followers' order across the hallway (the leftmost left, the next right, ...: nobody crosses
    // the other's way); (FOL2 r2) after that the side of his present Destination when it already is one (no swap once the file
    // stands, which walked a friend across the hallway)
    let off = (q.pos - k.centre).dot(left);
    let mut al = Vec::new();
    c.world.allies(&mut al);
    let left_of_me = al.iter().filter(|a| a.alive && a.actor != p.actor.actor && a.actor != q.actor && matches!(a.actor, ActorRef::Member(_))).filter(|a| (a.position - k.centre).dot(left) > off).count();
    let entering = if left_of_me % 2 == 0 { 1.0 } else { -1.0 };
    let dest = c.bb.key_id("Destination").map(|id| c.bb.vector(id)).filter(|d| crate::bb::is_valid_location(*d));
    let side = match dest.map(|d| (d - k.centre).dot(left)) {
        Some(s) if s.abs() > 0.5 * SLOT_STAGGER.min(half.max(0.0)) && s.abs() > 15.0 => s.signum(),
        _ => entering,
    };
    let lat = side * SLOT_STAGGER.min(half.max(0.0));
    let centre = Vec3::new(k.centre.x, k.centre.y, p.floor_position.z);
    let spot = centre + back * along + left * lat;
    // the floor under it (the nav's projection), else the spot itself
    Some(c.world.nav().project(spot, 100.0, 400.0).unwrap_or(spot))
}

/// The run_def post-pass: reachability of the follow-type queries' winner, the hallway slot of the follow queries.
pub fn post(c: &Ctx, query: &str, q: &Querier, out: &mut Vec<(Item, f32)>) {
    if !ON.load(std::sync::atomic::Ordering::Relaxed) {
        return;
    }
    if REACH_QUERIES.contains(&query) {
        let nav = c.world.nav();
        let from = c.world.player().floor_position;
        let mut checks = 0usize;
        // the best first: a reachable one ends the pass, an unreachable one is dropped and the next is asked
        while !out.is_empty() && checks < REACH_CHECKS {
            let to = out[0].0.pos;
            checks += 1;
            let ok = match nav.reachable_strict(from, to) {
                Some(false) => nav.reachable_strict(q.pos, to),
                other => other,
            };
            match ok {
                None | Some(true) => break,
                Some(false) => {
                    out.remove(0);
                    // none of the best REACH_CHECKS walkable: the query finds nothing (KH3: no navmesh item there)
                    if checks == REACH_CHECKS {
                        out.clear();
                    }
                }
            }
        }
    }
    // (FOL2 r2) the winner kept off the walls (KH3's navmesh is eroded by the agent's radius): 8 short rays at knee height, the
    // spot moved out of any wall nearer than his radius + WALL_MARGIN
    if REACH_QUERIES.contains(&query) {
        if let Some(first) = out.first_mut() {
            first.0.pos = clear_of_walls(c, first.0.pos, q.radius.max(30.0) + WALL_MARGIN);
        }
    }
    if query == "n_Common_RandomWalkAroundPlayer" {
        if let Some(first) = out.first() {
            c.shared.action_dest.lock().unwrap_or_else(|e| e.into_inner()).insert(q.member, (first.0.pos, c.st.time));
        }
    }
    if SLOT_QUERIES.contains(&query) {
        if let Some(s) = hallway_slot(c, q) {
            out.insert(0, (Item { pos: s, actor: ActorRef::None, comp: 0 }, f32::MAX));
        }
    }
}

/// (FOL2 r2) `p` moved out of the walls nearer than `room` (cm) round it (8 rays at knee height; a host without rays: `p`).
pub fn clear_of_walls(c: &Ctx, p: Vec3, room: f32) -> Vec3 {
    let nav = c.world.nav();
    let o = p + Vec3::Z * PROBE_UP;
    let mut shift = Vec3::ZERO;
    for k in 0..8 {
        let d = yaw_dir(45.0 * k as f32);
        if let Some(h) = nav.ray(o, o + d * room) {
            if h < room {
                shift -= d * (room - h);
            }
        }
    }
    // opposite walls (a hallway narrower than two rooms): the middle
    let s = Vec3::new(shift.x, shift.y, 0.0);
    if s.length_squared() < 1e-4 {
        return p;
    }
    p + s * 0.5
}
