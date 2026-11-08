//! (NOKRON-WALLS, THE USER 2026-11-07: "the Nokron fight needs invisible walls around the arena, so that the player can't
//! jump out through the windows (the invisible walls should be placed just in front of the walls and taller than the actual
//! walls of the arena (make sure the two entrances into the room can still be walked through without issue)") INVISIBLE
//! WALLS OF A BOSS ARENA, for every KH character: a small reusable system driven by the rows of `sheets/port/arena-walls.json`
//! (compiled in). State note and measurements: build/sora_tmp/nokron_walls/STATE.md.
//!
//! THE ROWS (`arena.<fight key>.*`, MSB metres of the arena's map; the fight key is the boss row's, e.g. kh-partyhost
//! `MimicBossRow::key` "mimic_tear"):
//! - `anchor` [entity, x, y, z]: a character of the map that stands at its MSB place (Nokron: the silver pool). Elden Ring's
//!   physics frame is the MSB's moved (no turn): physics = MSB - this place + the anchor's physics place, each frame.
//! - `floor` (MSB y), `bottom` / `top` (m above the floor): the walls' heights (Nokron: -3 .. 66 m; the real walls reach 37.3 m,
//!   KH3's reach is the `reach.*` rows).
//! - `wall.<n>` [x0, z0, x1, z1]: the real walls' axes, in order round the room (each starts where the last ended).
//! - `inset` (m): each invisible wall is its wall's axis moved this far into the room ("just in front of the walls"); the
//!   corners are where the moved lines cross, so the ring is closed.
//! - `gap.<name>` [wall, s0, s1, top]: a doorway, s along the wall's axis from its start: no invisible wall there from the
//!   bottom up to `top` (m above the floor); above it the invisible wall goes on. At both edges a short invisible wall
//!   (`return_past` m beyond the axis) closes the strip between the invisible wall and the real one.
//! - `window.<name>` [wall, s0, s1, sill, top]: documentation and the tests' targets only.
//!
//! THE ROUTE: every KH character's movement asks `kh_core::World` (sweeps, lines, overlaps), which `world.rs` `ErWorld`
//! answers with sphere casts (`sweep::SphereCaster`). The walls are one more caster's contacts in `ErWorld::cast`
//! (`cast` here), so the leader, the AI party and the Mimic copies (they all move through Sora's world) are stopped by them
//! like by any wall of the game; the camera has its own world and passes through. Their contacts carry the component ids
//! `BODY_BASE..` (`is_wall_body`), which have no traversal flags (no wall kick, no dangle from their top).
//!
//! WHEN: only while a fight with rows here runs (`fight`, told once a frame by the fight's own state: kh-er `mimic_boss.rs`,
//! `mimicboss::fight_on`) AND the leader stands in the room (`active`). Nothing else ever meets them.

use glam::{Vec2, Vec3};

use crate::sweep::Contact;

/// The rows (compiled in).
pub const SHEET: &str = include_str!("../../../sheets/port/arena-walls.json");

/// The walls' component ids (`Contact::body` / `Hit::component`): far above any Havok body's (+ 1).
pub const BODY_BASE: u32 = 0xA7E0_0000;
const BODY_SPAN: u32 = 0x1000;

/// Is this component one of the invisible walls?
pub fn is_wall_body(component: u32) -> bool {
    (BODY_BASE..BODY_BASE + BODY_SPAN).contains(&component)
}

/// A doorway of a wall.
#[derive(Clone, Debug, PartialEq)]
pub struct Gap {
    pub name: String,
    /// The wall (0-based, in the rows' order).
    pub wall: usize,
    /// Along the wall's axis from its start, m.
    pub s0: f32,
    pub s1: f32,
    /// Open from the walls' bottom up to this, m above the floor.
    pub top: f32,
}

/// A window of a wall (documentation; the tests' targets).
#[derive(Clone, Debug, PartialEq)]
pub struct Window {
    pub name: String,
    pub wall: usize,
    pub s0: f32,
    pub s1: f32,
    /// m above the floor
    pub sill: f32,
    /// m above the floor; None: open to the sky
    pub top: Option<f32>,
}

/// One arena's rows.
#[derive(Clone, Debug, PartialEq)]
pub struct Arena {
    pub key: String,
    pub anchor_entity: u32,
    /// The anchor's MSB place.
    pub anchor: Vec3,
    pub floor: f32,
    pub bottom: f32,
    pub top: f32,
    pub inset: f32,
    pub return_past: f32,
    /// The real walls' axes (MSB x, z): start, end.
    pub walls: Vec<(Vec2, Vec2)>,
    pub gaps: Vec<Gap>,
    pub windows: Vec<Window>,
}

/// A vertical rectangle: the horizontal segment `a`-`b` between the heights `lo` and `hi`. In MSB space the horizontal axes
/// are (x, z) and the height is y; in KH3 space (x, y) and z.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Panel {
    pub a: Vec2,
    pub b: Vec2,
    pub lo: f32,
    pub hi: f32,
    pub body: u32,
}

fn nums(v: &serde_json::Value) -> Vec<f32> {
    match v {
        serde_json::Value::Number(n) => n.as_f64().map(|x| vec![x as f32]).unwrap_or_default(),
        serde_json::Value::Array(items) => items.iter().filter_map(|i| i.as_f64().map(|x| x as f32)).collect(),
        _ => Vec::new(),
    }
}

/// Every arena of a sheet's text (the rows `arena.<key>.*`), checked: a closed ring of at least three walls, its gaps and
/// windows on them.
pub fn parse(text: &str) -> Result<Vec<Arena>, String> {
    let json: serde_json::Value = serde_json::from_str(text).map_err(|e| format!("arena-walls: {e}"))?;
    let rows = json["rows"].as_array().ok_or("arena-walls: no rows")?;
    let mut arenas: Vec<Arena> = Vec::new();
    let mut walls: Vec<(String, u32, Vec2, Vec2)> = Vec::new();
    for row in rows {
        let Some(id) = row[0].as_str() else { continue };
        let mut parts = id.splitn(4, '.');
        let (Some("arena"), Some(key), Some(field)) = (parts.next(), parts.next(), parts.next()) else { continue };
        let name = parts.next().unwrap_or("");
        let v = nums(&row[1]);
        if !arenas.iter().any(|a| a.key == key) {
            arenas.push(Arena { key: key.to_string(), anchor_entity: 0, anchor: Vec3::ZERO, floor: 0.0, bottom: 0.0, top: 0.0, inset: 0.0, return_past: 0.0, walls: Vec::new(), gaps: Vec::new(), windows: Vec::new() });
        }
        let Some(a) = arenas.iter_mut().find(|a| a.key == key) else { continue };
        let bad = || format!("arena-walls: {id}: {:?} is not what the row wants", row[1]);
        match (field, v.as_slice()) {
            ("anchor", [e, x, y, z]) => (a.anchor_entity, a.anchor) = (*e as u32, Vec3::new(*x, *y, *z)),
            ("floor", [x]) => a.floor = *x,
            ("bottom", [x]) => a.bottom = *x,
            ("top", [x]) => a.top = *x,
            ("inset", [x]) => a.inset = *x,
            ("return_past", [x]) => a.return_past = *x,
            ("wall_top", [_]) => {}
            ("wall", [x0, z0, x1, z1]) => walls.push((key.to_string(), name.parse::<u32>().map_err(|_| bad())?, Vec2::new(*x0, *z0), Vec2::new(*x1, *z1))),
            ("gap", [w, s0, s1, top]) if *w >= 1.0 => a.gaps.push(Gap { name: name.to_string(), wall: *w as usize - 1, s0: *s0, s1: *s1, top: *top }),
            ("window", [w, s0, s1, sill, top]) if *w >= 1.0 => a.windows.push(Window { name: name.to_string(), wall: *w as usize - 1, s0: *s0, s1: *s1, sill: *sill, top: (*top > 0.0).then_some(*top) }),
            _ => return Err(bad()),
        }
    }
    walls.sort_by(|x, y| x.0.cmp(&y.0).then(x.1.cmp(&y.1)));
    for a in arenas.iter_mut() {
        a.walls = walls.iter().filter(|w| w.0 == a.key).map(|w| (w.2, w.3)).collect();
        let n = a.walls.len();
        if n < 3 {
            return Err(format!("arena-walls: {}: {n} walls (a room needs 3 or more)", a.key));
        }
        for k in 0..n {
            let (end, next) = (a.walls[k].1, a.walls[(k + 1) % n].0);
            if end.distance(next) > 0.05 {
                return Err(format!("arena-walls: {}: wall {} ends at {end} but wall {} starts at {next}", a.key, k + 1, (k + 1) % n + 1));
            }
        }
        if a.top <= a.bottom || a.inset <= 0.0 || a.anchor_entity == 0 {
            return Err(format!("arena-walls: {}: top {} / bottom {} / inset {} / anchor {}", a.key, a.top, a.bottom, a.inset, a.anchor_entity));
        }
        for g in &a.gaps {
            let len = a.walls.get(g.wall).map_or(0.0, |w| w.0.distance(w.1));
            if g.wall >= n || g.s0 < 0.0 || g.s1 <= g.s0 || g.s1 > len || g.top <= a.bottom {
                return Err(format!("arena-walls: {}: gap {} [{} {} .. {}, top {}] is not on a wall", a.key, g.name, g.wall + 1, g.s0, g.s1, g.top));
            }
        }
    }
    Ok(arenas)
}

/// The arenas of the compiled sheet (an empty list if it is broken: no walls rather than wrong ones).
pub fn arenas() -> &'static [(Arena, Vec<Panel>)] {
    static ALL: std::sync::OnceLock<Vec<(Arena, Vec<Panel>)>> = std::sync::OnceLock::new();
    ALL.get_or_init(|| {
        parse(SHEET)
            .map(|list| {
                list.into_iter()
                    .map(|a| {
                        let panels = panels_msb(&a);
                        (a, panels)
                    })
                    .collect()
            })
            .unwrap_or_default()
    })
}

/// The arena of a fight key.
pub fn arena_of(key: &str) -> Option<&'static (Arena, Vec<Panel>)> {
    arenas().iter().find(|(a, _)| a.key == key)
}

fn cross(a: Vec2, b: Vec2) -> f32 {
    a.x * b.y - a.y * b.x
}

/// Twice the signed area of the axes' ring (x, z).
fn ring_area(a: &Arena) -> f32 {
    let n = a.walls.len();
    (0..n).map(|k| cross(a.walls[k].0, a.walls[(k + 1) % n].0)).sum()
}

/// A wall's direction and its normal into the room.
fn wall_frame(a: &Arena, k: usize) -> (Vec2, Vec2) {
    let (p, q) = a.walls[k];
    let d = (q - p).normalize_or_zero();
    let left = Vec2::new(-d.y, d.x);
    (d, if ring_area(a) > 0.0 { left } else { -left })
}

/// Where the invisible walls of walls `k - 1` and `k` meet (their moved lines' crossing).
fn corner(a: &Arena, k: usize) -> Vec2 {
    let n = a.walls.len();
    let j = (k + n - 1) % n;
    let (dj, nj) = wall_frame(a, j);
    let (dk, nk) = wall_frame(a, k);
    let (oj, ok) = (a.walls[j].0 + nj * a.inset, a.walls[k].0 + nk * a.inset);
    let den = cross(dj, dk);
    if den.abs() < 1e-6 {
        return ok;
    }
    oj + dj * (cross(ok - oj, dk) / den)
}

/// The invisible walls of an arena, MSB space: per wall its moved line from corner to corner, minus its gaps; over each gap
/// its lintel (from the gap's top up), and at each gap's edge the return to the real wall.
pub fn panels_msb(a: &Arena) -> Vec<Panel> {
    let n = a.walls.len();
    let (lo, hi) = (a.floor + a.bottom, a.floor + a.top);
    let mut out: Vec<Panel> = Vec::new();
    let mut push = |p: Vec2, q: Vec2, lo: f32, hi: f32| {
        if p.distance(q) > 1e-3 && hi > lo {
            let body = BODY_BASE + out.len() as u32;
            out.push(Panel { a: p, b: q, lo, hi, body });
        }
    };
    for k in 0..n {
        let (d, nrm) = wall_frame(a, k);
        let start = a.walls[k].0;
        let at = |s: f32| start + nrm * a.inset + d * s;
        let (t0, t1) = ((corner(a, k) - start).dot(d), (corner(a, (k + 1) % n) - start).dot(d));
        let mut gaps: Vec<&Gap> = a.gaps.iter().filter(|g| g.wall == k).collect();
        gaps.sort_by(|x, y| x.s0.total_cmp(&y.s0));
        let mut from = t0;
        for g in &gaps {
            if g.s0 > from {
                push(at(from), at(g.s0.min(t1)), lo, hi);
            }
            from = from.max(g.s1);
            push(at(g.s0), at(g.s1), a.floor + g.top, hi);
            for s in [g.s0, g.s1] {
                push(at(s), start + d * s - nrm * a.return_past, lo, hi);
            }
        }
        if from < t1 {
            push(at(from), at(t1), lo, hi);
        }
    }
    out
}

/// Is an MSB place inside the room (the real walls' ring, between the walls' bottom and top)?
pub fn inside(a: &Arena, p: Vec3) -> bool {
    if p.y < a.floor + a.bottom || p.y > a.floor + a.top {
        return false;
    }
    let q = Vec2::new(p.x, p.z);
    let n = a.walls.len();
    let mut odd = false;
    for k in 0..n {
        let (u, v) = (a.walls[k].0, a.walls[(k + 1) % n].0);
        if (u.y > q.y) != (v.y > q.y) && q.x < u.x + (q.y - u.y) / (v.y - u.y) * (v.x - u.x) {
            odd = !odd;
        }
    }
    odd
}

/// THE RULE: the walls stand while the arena's fight runs and the leader (his MSB place) is in its room.
pub fn active(a: &Arena, fight_on: bool, leader_msb: Vec3) -> bool {
    fight_on && inside(a, leader_msb)
}

/// The panels moved to another space by `to` (an MSB place -> that space's place, KH3: horizontal (x, y), height z). The map
/// must keep up as up (a relabelling, a scale, a move: `coords::Frame`'s).
pub fn to_space(panels: &[Panel], to: impl Fn(Vec3) -> Vec3) -> Vec<Panel> {
    panels
        .iter()
        .map(|p| {
            let (a, b) = (to(Vec3::new(p.a.x, p.lo, p.a.y)), to(Vec3::new(p.b.x, p.hi, p.b.y)));
            Panel { a: Vec2::new(a.x, a.y), b: Vec2::new(b.x, b.y), lo: a.z.min(b.z), hi: a.z.max(b.z), body: p.body }
        })
        .collect()
}

/// The point of a panel nearest to `q` (KH3 space: z up).
fn closest(p: &Panel, q: Vec3) -> Vec3 {
    let ab = p.b - p.a;
    let len2 = ab.length_squared();
    let t = if len2 > 0.0 { ((Vec2::new(q.x, q.y) - p.a).dot(ab) / len2).clamp(0.0, 1.0) } else { 0.0 };
    let h = p.a + ab * t;
    Vec3::new(h.x, h.y, q.z.clamp(p.lo, p.hi))
}

/// The panel's face normal on the side of `q` (or against `delta` when `q` is on it).
fn face(p: &Panel, q: Vec3, delta: Vec3) -> Vec3 {
    let d = (p.b - p.a).normalize_or_zero();
    let n = Vec3::new(-d.y, d.x, 0.0);
    let side = (Vec2::new(q.x, q.y) - p.a).dot(Vec2::new(n.x, n.y));
    if side > 0.0 || (side == 0.0 && n.dot(delta) < 0.0) {
        n
    } else {
        -n
    }
}

/// The first contact of a sphere of `radius` moved from `start` by `delta` with one panel (KH3 space), as the host physics
/// reports one: fraction 0 when it starts inside; the touched point; the normal from the point to the sphere's centre.
pub fn first_contact(p: &Panel, start: Vec3, delta: Vec3, radius: f32) -> Option<Contact> {
    // the swept sphere's box against the panel's
    let (lo, hi) = (start.min(start + delta) - Vec3::splat(radius), start.max(start + delta) + Vec3::splat(radius));
    let (plo, phi) = (Vec3::new(p.a.x.min(p.b.x), p.a.y.min(p.b.y), p.lo), Vec3::new(p.a.x.max(p.b.x), p.a.y.max(p.b.y), p.hi));
    if lo.cmpgt(phi).any() || hi.cmplt(plo).any() {
        return None;
    }
    let dist = |t: f32| {
        let c = start + delta * t;
        c.distance(closest(p, c))
    };
    let contact = |t: f32| {
        let c = start + delta * t;
        let point = closest(p, c);
        let normal = (c - point).try_normalize().unwrap_or_else(|| face(p, c, delta));
        Contact { fraction: t, point, normal, body: p.body }
    };
    if dist(0.0) <= radius {
        return Some(contact(0.0));
    }
    // the distance along a straight move to a convex shape is convex: its least by golden section, then the first touch
    let (mut a, mut b) = (0.0f32, 1.0f32);
    const G: f32 = 0.618_034;
    for _ in 0..48 {
        let (m1, m2) = (b - (b - a) * G, a + (b - a) * G);
        if dist(m1) <= dist(m2) {
            b = m2;
        } else {
            a = m1;
        }
    }
    let least = (a + b) * 0.5;
    let least = if dist(1.0) < dist(least) { 1.0 } else { least };
    if dist(least) > radius {
        return None;
    }
    let (mut out, mut touch) = (0.0f32, least);
    for _ in 0..40 {
        let mid = (out + touch) * 0.5;
        if dist(mid) <= radius {
            touch = mid;
        } else {
            out = mid;
        }
    }
    Some(contact(touch))
}

/// Every panel's first contact of a sphere cast (KH3 space), added to `out` (the host's own contacts stay).
pub fn cast(panels: &[Panel], start: Vec3, delta: Vec3, radius: f32, out: &mut Vec<Contact>) {
    out.extend(panels.iter().filter_map(|p| first_contact(p, start, delta, radius)));
}

/// What the fight's frame told this frame: (its key, it runs).
static TOLD: std::sync::Mutex<Option<(&'static str, bool)>> = std::sync::Mutex::new(None);

/// ONCE A FRAME from a fight's own state (kh-er `mimic_boss.rs`): the fight `key` runs (`on`) or not. Walls stand only on a
/// frame this said "on" (`frame` takes it).
pub fn fight(key: &'static str, on: bool) {
    if let Ok(mut g) = TOLD.lock() {
        *g = Some((key, on));
    }
}

fn take_told() -> Option<(&'static str, bool)> {
    TOLD.lock().ok().and_then(|mut g| g.take())
}

#[cfg(windows)]
pub use live::frame;

#[cfg(windows)]
mod live {
    use glam::Vec3;

    use super::{active, arena_of, take_told, to_space};
    use crate::erlog;
    use crate::world::ErWorld;

    /// The last state logged (a line is written when it changes).
    static SAID: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());

    fn say(state: String, line: impl FnOnce() -> String) {
        if let Ok(mut g) = SAID.lock() {
            if *g != state {
                erlog!("{}", line());
                *g = state;
            }
        }
    }

    /// ONCE A FRAME (sora.rs, after the bosses' frame, before anyone moves): the walls of the fight told on this frame, moved
    /// to KH3 space by the anchor's physics place, into Sora's world; none otherwise. `leader_kh`: the leader's feet.
    pub fn frame(world: &ErWorld, leader_kh: Vec3) {
        let told = take_told();
        let mut walls = world.arena.borrow_mut();
        let Some((key, on)) = told.filter(|t| t.1) else {
            if !walls.is_empty() {
                walls.clear();
                say("down".to_string(), || format!("arenawalls: {}: the invisible walls are down", told.map_or("no fight in the loaded world (or no party host)".to_string(), |t| format!("{}: the fight does not run", t.0))));
            }
            return;
        };
        let Some((arena, panels)) = arena_of(key) else {
            walls.clear();
            return;
        };
        let Some(anchor_er) = crate::zphase::chr_of_entity(arena.anchor_entity).map(|c| crate::game::v3(&c.modules.physics.position)) else {
            walls.clear();
            say(format!("no anchor {key}"), || format!("!!! arenawalls: {key}: the fight runs but its anchor (entity {}) is not in the loaded world: NO invisible walls", arena.anchor_entity));
            return;
        };
        // physics = MSB - the anchor's MSB place + its physics place (no turn)
        let shift = anchor_er - arena.anchor;
        let leader_msb = world.frame.pos_kh_to_er(leader_kh) - shift;
        if !active(arena, on, leader_msb) {
            walls.clear();
            say(format!("outside {key}"), || format!("arenawalls: {key}: the fight runs, the leader outside the room (MSB ({:.2}, {:.2}, {:.2})): the invisible walls are down", leader_msb.x, leader_msb.y, leader_msb.z));
            return;
        }
        *walls = to_space(panels, |m: Vec3| world.frame.pos_er_to_kh(m + shift));
        say(format!("up {key}"), || format!("arenawalls: {key}: THE INVISIBLE WALLS STAND ({} panels, {:.1} .. {:.1} m above the floor, {:.1} m in front of the walls; {} doorway(s) open): the leader in the room at MSB ({:.2}, {:.2}, {:.2}); the anchor (entity {}) at Elden Ring ({:.2}, {:.2}, {:.2})", panels.len(), arena.bottom, arena.top, arena.inset, arena.gaps.len(), leader_msb.x, leader_msb.y, leader_msb.z, arena.anchor_entity, anchor_er.x, anchor_er.y, anchor_er.z));
    }
}

#[cfg(test)]
#[path = "arena_walls_tests.rs"]
mod tests;
