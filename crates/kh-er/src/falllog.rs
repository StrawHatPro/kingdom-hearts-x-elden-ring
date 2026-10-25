//! (IMPL_ER24) WHY HE LEFT THE FLOOR / WHAT HE PUSHES AGAINST: a few log lines at the moment it happens, cheap enough to
//! be on by default (`log.floor = 0` / `log.blocked = 0` silence them; at most one report a second, 300 a session).
//! Nothing here changes his movement or the game.
//!
//! - `left_floor`: at a change Walking -> Falling that is no jump: what KH3's floor finding gets through the adapter
//!   (the body's sweep, the line under his centre, the foot rule), the RAW contacts of the game's physics under him on his
//!   own layer and on the floor layer (height from his feet, where, normal, body, owner), and A LAYER SURVEY: which layers
//!   of the game's collision filter see a floor under him within reach, for a thin and for a foot-wide sphere. A floor
//!   that only other layers see = his casts use a layer that lacks it (the question the user's log of 2026-10-13 left
//!   open at the door: the planks end 69 cm before the wall and nothing answered his casts there).
//! - `blocked`: on the floor, pushing, not moving for half a second: what is ahead (raw, both layers), the body's sweep
//!   ahead, the three sweeps of KH3's StepUp (up 47, ahead, down 51.8) repeated one by one with their results, and the
//!   layer survey ahead.

use std::collections::BTreeMap;

use glam::Vec3;
use kh_core::{Hit, Output, Sora, World, SORA_PARAMS};

use crate::coords::{dir_kh_to_er, unit_er_to_kh, CM_PER_M};
use crate::erlog;
use crate::havok::{self, CastHit};
use crate::sweep;
use crate::wallscan::{owner_text, ranges};
use crate::world::{ErWorld, CAST_MARGIN_M};

fn hit_text(hit: Option<Hit>, feet: Vec3) -> String {
    hit.map_or("nothing".to_string(), |h| {
        format!(
            "t {:.3} at {:+.1} cm from his feet, ({:+.0}, {:+.0}) from his axis, normal ({:.2}, {:.2}, {:.2}) surface ({:.2}, {:.2}, {:.2}){} body {}",
            h.time, h.position.z - feet.z, h.position.x - feet.x, h.position.y - feet.y, h.normal.x, h.normal.y, h.normal.z, h.impact_normal.x, h.impact_normal.y, h.impact_normal.z,
            if h.start_penetrating { format!(" IN OVERLAP {:.1} cm", h.penetration_depth) } else { String::new() },
            h.component,
        )
    })
}

/// The raw contacts of one sphere cast (KH3 cm in, the game's physics asked directly), nearest first.
fn raw(world: &ErWorld, filter: u32, start: Vec3, delta: Vec3, radius: f32) -> Vec<CastHit> {
    let mut hits = Vec::new();
    havok::cast_sphere(world.frame.pos_kh_to_er(start), dir_kh_to_er(delta), (radius / CM_PER_M - CAST_MARGIN_M).max(0.001), filter, false, &mut hits);
    hits.retain(|h| h.fraction.is_finite());
    hits.sort_by(|a, b| a.fraction.total_cmp(&b.fraction));
    hits
}

fn raw_text(world: &ErWorld, hits: &[CastHit], feet: Vec3) -> String {
    if hits.is_empty() {
        return "NOTHING".to_string();
    }
    let mut bodies: Vec<u32> = hits.iter().map(|h| h.body).collect();
    bodies.sort();
    bodies.dedup();
    let mut s = format!("{} contact(s) on {} bodies:", hits.len(), bodies.len());
    for h in hits.iter().take(5) {
        let p = world.frame.pos_er_to_kh(h.position);
        let n = unit_er_to_kh(h.normal);
        s.push_str(&format!(" [f {:.3} {:+.1} cm at ({:+.0}, {:+.0}) n ({:.2}, {:.2}, {:.2}) body {} {}]", h.fraction, p.z - feet.z, p.x - feet.x, p.y - feet.y, n.x, n.y, n.z, h.body, owner_text(h.owner_ptr)));
    }
    s
}

/// One cast with every layer 0..=127: the layers grouped by the first contact `keep` accepts (said by `say`).
fn survey(world: &ErWorld, start: Vec3, delta: Vec3, radius: f32, keep: impl Fn(&CastHit) -> bool, say: impl Fn(&CastHit) -> String) -> String {
    let mut groups: BTreeMap<String, Vec<u32>> = BTreeMap::new();
    for layer in 0..128u32 {
        let hits = raw(world, (world.filter & !0xFF) | layer, start, delta, radius);
        let what = hits.iter().find(|h| keep(h)).map_or("nothing".to_string(), &say);
        groups.entry(what).or_default().push(layer);
    }
    groups.iter().map(|(what, layers)| format!("{what}: {}", ranges(layers))).collect::<Vec<_>>().join(" | ")
}

/// He left the floor without jumping. `before`: the step before (on the floor), `now`: this one.
pub fn left_floor(world: &ErWorld, before: &Output, now: &Output) {
    let p = &SORA_PARAMS;
    let height = Sora::origin_height_on_floor(p);
    let feet = now.position - Vec3::Z * height;
    let er = world.frame.pos_kh_to_er(now.position);
    let opts = world.opts.get();
    erlog!(
        "floor: HE LEFT THE FLOOR WITHOUT A JUMP at step {} kh ({:.1}, {:.1}, {:.1}) = Elden Ring ({:.2}, {:.2}, {:.2}), state {:?}/{}, vel ({:.0}, {:.0}, {:.0}); the step before he stood at ({:+.1}, {:+.1}, {:+.1}) from here | adapter {:?}, his layer {:#x}, floor layer {:#x}, body radius {:.0}",
        now.step, now.position.x, now.position.y, now.position.z, er.x, er.y, er.z, now.state, now.sub_step, now.velocity.x, now.velocity.y, now.velocity.z,
        before.position.x - now.position.x, before.position.y - now.position.y, before.position.z - now.position.z, opts, world.filter & 0xFF, world.floor_filter.get() & 0xFF, p.capsule_radius.min(world.radius_cap.get()),
    );
    // what KH3's floor finding is told (FindFloor: the body 49.4 down; the line from the origin; the perch probe's reach 59.4)
    let centre = now.capsule_center;
    let body = world.sweep_capsule(centre, centre - Vec3::Z * 49.4, p.capsule_radius, p.capsule_half_height);
    let line = world.line_trace(now.position, now.position - Vec3::Z * 59.4);
    let plain = sweep::line_first(world, now.position, now.position - Vec3::Z * 59.4, &sweep::Options::PLAIN);
    let bottom = centre - Vec3::Z * p.capsule_half_height;
    let foot = sweep::foot_support(world, bottom, 59.4, &opts);
    erlog!(
        "floor:   KH3 is told | the body's sweep 49 down: {} | the line under his centre 59 down: {} (the bare line: {}) | the foot rule (radius {:.0}): {}",
        hit_text(body, feet), hit_text(line, feet), hit_text(plain, feet), opts.foot,
        foot.map_or("NOT HELD (no rim under the foot, or nothing on the far side of it: a ledge or a hole wider than the foot)".to_string(), |(z, b)| format!("HELD at {:+.1} cm from his feet by body {b}", z - feet.z)),
    );
    // the game's physics asked directly, both layers
    let r = p.capsule_radius.min(world.radius_cap.get());
    for (name, filter) in [("his layer", world.filter), ("the floor layer", world.floor_filter.get())] {
        if filter == 0 {
            continue;
        }
        let sphere = raw(world, filter, feet + Vec3::Z * (r + 10.0), Vec3::NEG_Z * 70.0, r);
        let wide = raw(world, filter, feet + Vec3::Z * (opts.foot.max(r) + 10.0), Vec3::NEG_Z * 70.0, opts.foot.max(r));
        let thin = raw(world, filter, feet + Vec3::Z * 50.0, Vec3::NEG_Z * 350.0, 0.5 + CAST_MARGIN_M * CM_PER_M);
        erlog!(
            "floor:   RAW, {name} {:#x} | a sphere of {r:.0} from 10 above his feet 70 down: {} | a sphere of {:.0}: {} | a line from 50 above 350 down: {}",
            filter & 0xFF, raw_text(world, &sphere, feet), opts.foot.max(r), raw_text(world, &wide, feet), raw_text(world, &thin, feet),
        );
    }
    // which layers see a floor under him
    let floorish = |h: &CastHit| h.normal.y > 0.3 && h.fraction > 1.0e-5;
    let say = |h: &CastHit| format!("{:+.0} cm ({}{})", (world.frame.pos_er_to_kh(h.position).z - feet.z).round(), if h.owner == 8 { "map" } else if h.owner == 6 { "object" } else { "owner " }, if h.owner == 8 || h.owner == 6 { String::new() } else { h.owner.to_string() });
    erlog!("floor:   LAYER SURVEY, a sphere of 10 on his axis from 40 above his feet 110 down, the first floor each layer sees: {}", survey(world, feet + Vec3::Z * 50.0, Vec3::NEG_Z * 110.0, 10.0, floorish, say));
    erlog!("floor:   LAYER SURVEY, a sphere of 40 the same way: {}", survey(world, feet + Vec3::Z * 80.0, Vec3::NEG_Z * 110.0, 40.0, floorish, say));
}

/// He pushes along `velocity` on the floor and does not move.
pub fn blocked(world: &ErWorld, now: &Output) {
    let p = &SORA_PARAMS;
    let height = Sora::origin_height_on_floor(p);
    let feet = now.position - Vec3::Z * height;
    let er = world.frame.pos_kh_to_er(now.position);
    let dir = Vec3::new(now.velocity.x, now.velocity.y, 0.0).normalize_or_zero();
    if dir == Vec3::ZERO {
        return;
    }
    let centre = now.capsule_center;
    let (r, hh) = (p.capsule_radius, p.capsule_half_height);
    erlog!(
        "blocked: ON THE FLOOR, PUSHING ALONG ({:.2}, {:.2}) AT {:.0} cm/s AND NOT MOVING at step {} kh ({:.1}, {:.1}, {:.1}) = Elden Ring ({:.2}, {:.2}, {:.2}), state {:?}/{} | adapter {:?}, his layer {:#x}, floor layer {:#x}",
        dir.x, dir.y, now.velocity.truncate().length(), now.step, now.position.x, now.position.y, now.position.z, er.x, er.y, er.z, now.state, now.sub_step, world.opts.get(), world.filter & 0xFF, world.floor_filter.get() & 0xFF,
    );
    // KH3's own moves, one by one (MoveAlongFloor's sweep; StepUp: up 47, ahead, down 51.8)
    let ahead = world.sweep_capsule(centre, centre + dir * 30.0, r, hh);
    let up_hit = world.sweep_capsule(centre, centre + Vec3::Z * p.floor_max_step_height, r, hh);
    let up = p.floor_max_step_height * up_hit.map_or(1.0, |h| h.time);
    let raised = centre + Vec3::Z * (up - 0.2).max(0.0);
    let over_hit = world.sweep_capsule(raised, raised + dir * 30.0, r, hh);
    let over = 30.0 * over_hit.map_or(1.0, |h| h.time);
    let above = raised + dir * (over - 0.2).max(0.0);
    let down_hit = world.sweep_capsule(above, above - Vec3::Z * (p.floor_max_step_height + p.step_up_down_extra), r, hh);
    let floor_under = world.sweep_capsule(centre, centre - Vec3::Z * 49.4, r, hh);
    erlog!(
        "blocked:   KH3's moves | the floor under him: {} | his body 30 ahead: {} | StepUp: up {:.0} of {:.0} ({}), then {:.0} of 30 ahead ({}), then down: {}{}",
        hit_text(floor_under, feet), hit_text(ahead, feet), up, p.floor_max_step_height, hit_text(up_hit, feet), over, hit_text(over_hit, feet), hit_text(down_hit, feet),
        match down_hit {
            None => " => NO LANDING within 51.8".to_string(),
            Some(h) if h.start_penetrating => " => the landing sweep STARTS IN OVERLAP: StepUp refuses".to_string(),
            Some(h) => format!(" => a landing {:+.1} cm above his feet, {} (walkable from {:.2})", h.position.z - feet.z, if h.impact_normal.z >= p.floor_walkable_z { "WALKABLE" } else { "NOT WALKABLE" }, p.floor_walkable_z),
        },
    );
    let cap = r.min(world.radius_cap.get());
    for (name, filter) in [("his layer", world.filter), ("the floor layer", world.floor_filter.get())] {
        if filter == 0 {
            continue;
        }
        let low = raw(world, filter, feet + Vec3::Z * (cap + 3.0), dir * 60.0, cap);
        let high = raw(world, filter, feet + Vec3::Z * 120.0, dir * 60.0, cap);
        let down = raw(world, filter, feet + dir * 45.0 + Vec3::Z * 120.0, Vec3::NEG_Z * 180.0, 10.0);
        erlog!(
            "blocked:   RAW, {name} {:#x} | his lowest sphere 60 ahead: {} | a sphere at chest height 60 ahead: {} | a sphere of 10 straight down 45 ahead of him (from 120 above his feet): {}",
            filter & 0xFF, raw_text(world, &low, feet), raw_text(world, &high, feet), raw_text(world, &down, feet),
        );
    }
    let say_ahead = |h: &CastHit| {
        let n = unit_er_to_kh(h.normal);
        format!("{:.0} cm ahead at {:+.0} cm, n z {:.1} ({})", h.fraction * 80.0, (world.frame.pos_er_to_kh(h.position).z - feet.z).round(), n.z, if h.owner == 8 { "map" } else if h.owner == 6 { "object" } else { "other" })
    };
    erlog!("blocked:   LAYER SURVEY, a sphere of 15 from 25 above his feet 80 ahead, what each layer meets first: {}", survey(world, feet + Vec3::Z * 25.0, dir * 80.0, 15.0, |_| true, say_ahead));
    let say_down = |h: &CastHit| format!("{:+.0} cm ({})", (world.frame.pos_er_to_kh(h.position).z - feet.z).round(), if h.owner == 8 { "map" } else if h.owner == 6 { "object" } else { "other" });
    erlog!("blocked:   LAYER SURVEY, a sphere of 10 straight down 45 ahead of him (from 120 above his feet, 180), the first floor each layer sees: {}", survey(world, feet + dir * 45.0 + Vec3::Z * 120.0, Vec3::NEG_Z * 180.0, 10.0, |h| h.normal.y > 0.3, say_down));
}
