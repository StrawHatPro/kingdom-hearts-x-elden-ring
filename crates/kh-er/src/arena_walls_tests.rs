//! (NOKRON-WALLS) The invisible walls of the Nokron Mimic Tear's arena, from the real rows (sheets/port/arena-walls.json),
//! through the real collision route (the sphere casts `ErWorld::cast` adds them to, the PhysX adapter `sweep.rs`) and KH3's own
//! Sora (kh-core, the level-99 loadout: High Jump, Doubleflight, Air Slide, Glide, Aerial Dodge, the aerial combo) jumping,
//! dashing, gliding and attacking at every window, and walking through both doorways. The test room has the floor and the
//! invisible walls only (no real wall): what holds him is the invisible walls alone.

use super::*;
use crate::coords::Frame;
use crate::sweep::{self, Options, SphereCaster};
use kh_core::world::surface;
use kh_core::{Hit, Input, PadButton, PadButtons, PadState, Pose, Sora, World, SORA_PARAMS};

const DT: f32 = 1.0 / 60.0;
/// The widest cast of Sora's body in Elden Ring (sheet sora-er-host `collision.radius`).
const CAP: f32 = 32.0;

fn nokron() -> Arena {
    parse(SHEET).unwrap().into_iter().find(|a| a.key == "mimic_tear").expect("the Nokron arena's rows")
}

/// The Nokron rows' own numbers (the sheet's reach.* and wall_top rows).
fn row(id: &str) -> Vec<f32> {
    let json: serde_json::Value = serde_json::from_str(SHEET).unwrap();
    json["rows"].as_array().unwrap().iter().find(|r| r[0] == id).map(|r| nums(&r[1])).unwrap_or_else(|| panic!("{id}"))
}

/// A physics frame like the user's after the origin shift: MSB + (984, -616, 1160) = Elden Ring (MIMIC-BOSS MB3-FOGRISE).
struct Space {
    shift: Vec3,
    frame: Frame,
}

impl Space {
    fn new(a: &Arena) -> Space {
        let shift = Vec3::new(-984.0, 616.0, -1160.0);
        Space { shift, frame: Frame::at(a.anchor + shift + Vec3::new(3.0, 0.0, -2.0)) }
    }
    fn kh(&self, msb: Vec3) -> Vec3 {
        self.frame.pos_er_to_kh(msb + self.shift)
    }
    fn msb(&self, kh: Vec3) -> Vec3 {
        self.frame.pos_kh_to_er(kh) - self.shift
    }
    /// A horizontal MSB direction (x, z) as a KH3 yaw, degrees.
    fn yaw(&self, dir: Vec2) -> f32 {
        let k = crate::coords::dir_er_to_kh(Vec3::new(dir.x, 0.0, dir.y));
        k.y.atan2(k.x).to_degrees()
    }
}

/// The test room: the arena's floor and (optionally) its invisible walls, answered as `ErWorld` answers (sphere casts through
/// the adapter, Sora's casts capped at `CAP`, the walls without traversal flags).
struct Room {
    panels: Vec<Panel>,
    floor: f32,
    opts: Options,
}

impl Room {
    fn new(a: &Arena, space: &Space, walls: bool) -> Room {
        let panels = if walls { to_space(&panels_msb(a), |m| space.kh(m)) } else { Vec::new() };
        Room { panels, floor: space.kh(Vec3::new(a.anchor.x, a.floor, a.anchor.z)).z, opts: Options::DEFAULT }
    }
}

/// The level floor's contact (world.rs `flat_contact`).
fn floor_contact(floor: f32, start: Vec3, delta: Vec3, radius: f32) -> Option<Contact> {
    let gap = start.z - radius - floor;
    if gap <= 0.0 {
        return Some(Contact { fraction: 0.0, point: Vec3::new(start.x, start.y, floor), normal: Vec3::Z, body: 1 });
    }
    if delta.z >= 0.0 {
        return None;
    }
    let fraction = gap / -delta.z;
    (fraction <= 1.0).then(|| {
        let at = start + delta * fraction;
        Contact { fraction, point: Vec3::new(at.x, at.y, floor), normal: Vec3::Z, body: 1 }
    })
}

impl SphereCaster for Room {
    fn cast(&self, start: Vec3, delta: Vec3, radius: f32, out: &mut Vec<Contact>) {
        out.clear();
        out.extend(floor_contact(self.floor, start, delta, radius));
        cast(&self.panels, start, delta, radius, out);
    }
}

impl World for Room {
    fn sweep_capsule(&self, start: Vec3, end: Vec3, radius: f32, half_height: f32) -> Option<Hit> {
        if radius <= 0.0 && half_height <= 0.0 {
            return sweep::line_first(self, start, end, &self.opts);
        }
        sweep::sweep_first(self, start, end, radius.min(CAP), half_height, &self.opts)
    }
    fn line_trace(&self, start: Vec3, end: Vec3) -> Option<Hit> {
        sweep::line_first(self, start, end, &self.opts)
    }
    fn sweep_capsule_all(&self, start: Vec3, end: Vec3, radius: f32, half_height: f32) -> Vec<Hit> {
        sweep::sweep_all(self, start, end, radius.min(CAP), half_height, &self.opts)
    }
    fn overlap_sphere(&self, center: Vec3, radius: f32) -> bool {
        let mut out = Vec::new();
        SphereCaster::cast(self, center + Vec3::Z * 0.2, Vec3::Z * -0.2, radius, &mut out);
        !out.is_empty()
    }
    fn surface_flags(&self, component: u32) -> u32 {
        if is_wall_body(component) {
            0
        } else {
            surface::MESH_DEFAULT
        }
    }
}

/// Sora with the level-99 loadout (every ability: High Jump, Doubleflight, Air Slide, Glide, Aerial Dodge ...).
fn level99_sora(feet: Vec3, yaw: f32) -> Sora {
    let path = format!("{}/../../sheets/port/sora-loadout-99.json", env!("CARGO_MANIFEST_DIR"));
    let json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))).unwrap();
    let mut loadout = kh_core::Loadout::new("sora-loadout-99");
    for row in json["rows"].as_array().unwrap() {
        use kh_core::status::LoadoutValue as V;
        let value = match &row[1] {
            serde_json::Value::Number(n) => V::Number(n.as_f64().unwrap()),
            serde_json::Value::String(t) => V::Text(t.clone()),
            serde_json::Value::Array(items) => V::List(items.iter().filter_map(|i| i.as_str().map(str::to_string)).collect()),
            serde_json::Value::Object(map) => V::Map(map.iter().filter_map(|(k, v)| v.as_f64().map(|n| (k.clone(), n))).collect()),
            _ => continue,
        };
        loadout.set(row[0].as_str().unwrap(), &value).unwrap_or_else(|e| panic!("{e}"));
    }
    loadout.resolve().unwrap();
    let origin = feet + Vec3::Z * Sora::origin_height_on_floor(&SORA_PARAMS);
    Sora::new(SORA_PARAMS, Pose { position: origin, yaw }).with_status(kh_core::Status::from_loadout(&loadout))
}

fn pad(yaw: f32, pressed: &[PadButton], held: &[PadButton]) -> Input {
    let set = |list: &[PadButton]| list.iter().fold(PadButtons::NONE, |s, b| s.with(*b));
    let pressed = set(pressed);
    Input { move_stick: Vec2::new(0.0, 1.0), camera_yaw: yaw, ..Input::default() }.with_pad(PadState { held: PadButtons(set(held).0 | pressed.0), pressed, released: PadButtons::NONE })
}

/// The ways out KH3 gives him, each a script of the pad (the stick always toward the window).
#[derive(Clone, Copy, Debug, PartialEq)]
enum Way {
    /// High Jump held, Doubleflight at its top, then the glide (jump held in the fall).
    JumpGlide,
    /// High Jump, Doubleflight, then the Air Slide (guard + stick) at the second top, then the glide.
    AirSlide,
    /// In battle: High Jump, Doubleflight, then the aerial combo (Attack every 8 steps), then the Aerial Dodge, then more.
    AerialCombo,
}

const WAYS: [Way; 3] = [Way::JumpGlide, Way::AirSlide, Way::AerialCombo];

/// Runs a way out from `feet` toward `yaw` for 4 s; every place of his (KH3 actor origin, the capsule's centre).
fn run(room: &Room, feet: Vec3, yaw: f32, way: Way) -> Vec<Vec3> {
    let mut sora = level99_sora(feet, yaw);
    sora.force_battle(way == Way::AerialCombo);
    let (j, g, a) = (PadButton::JUMP, PadButton::GUARD, PadButton::ATTACK);
    let mut places: Vec<Vec3> = Vec::new();
    let (mut phase, mut since) = (0u8, 0u32);
    for step in 0..240u32 {
        let vz = places.len().checked_sub(2).map_or(0.0, |k: usize| (places[k + 1] - places[k]).z / DT);
        let input = match phase {
            0 if step < 4 => pad(yaw, &[], &[]),
            0 => {
                phase = 1;
                since = 0;
                pad(yaw, &[j], &[])
            }
            // rising: held until the first top (Doubleflight needs Velocity.Z <= 500)
            1 if since > 6 && vz <= 400.0 => {
                phase = 2;
                since = 0;
                pad(yaw, &[j], &[])
            }
            1 => pad(yaw, &[], &[j]),
            2 if since > 8 && vz <= 0.0 => {
                phase = 3;
                since = 0;
                match way {
                    Way::JumpGlide => pad(yaw, &[], &[j]),
                    Way::AirSlide => pad(yaw, &[g], &[]),
                    Way::AerialCombo => pad(yaw, &[a], &[]),
                }
            }
            2 => pad(yaw, &[], &[]),
            _ => {
                let only = |on: bool, b: PadButton| if on { vec![b] } else { Vec::new() };
                match way {
                    Way::JumpGlide => pad(yaw, &only(since == 2, j), &[j]),
                    Way::AirSlide => pad(yaw, &only(since == 50, j), &only(since > 50, j)),
                    Way::AerialCombo if since == 70 => pad(yaw, &[g], &[]),
                    Way::AerialCombo => pad(yaw, &only(since % 8 == 0, a), &[]),
                }
            }
        };
        since += 1;
        let out = sora.step(DT, &input, room);
        places.push(out.position);
    }
    places
}

/// How far beyond the invisible wall of `wall` a place is (m; negative = in front of it, in the room).
fn beyond(a: &Arena, wall: usize, space: &Space, kh: Vec3) -> f32 {
    let (_, n) = wall_frame(a, wall);
    let line = a.walls[wall].0 + n * a.inset;
    let p = space.msb(kh);
    -(Vec2::new(p.x, p.z) - line).dot(n)
}

/// The MSB place on a wall's axis at s, moved `into` m into the room, at `up` m above the floor.
fn on_wall(a: &Arena, wall: usize, s: f32, into: f32, up: f32) -> Vec3 {
    let (d, n) = wall_frame(a, wall);
    let p = a.walls[wall].0 + d * s + n * into;
    Vec3::new(p.x, a.floor + up, p.y)
}

// ---------------------------------------------------------------------------------------------------- the rows

#[test]
fn the_nokron_rows_are_a_closed_room_of_six_walls_two_doorways_and_twelve_windows() {
    let a = nokron();
    assert_eq!((a.walls.len(), a.gaps.len(), a.windows.len()), (6, 2, 12));
    assert_eq!((a.anchor_entity, a.anchor), (12_020_860, Vec3::new(1001.327, -617.556, 1142.989)), "the pool, kh-partyhost MimicBossRow.pool_at");
    // the fight key is the boss row's
    assert!(kh_partyhost_key_is("mimic_tear"));
    // taller than the real walls by more than KH3's reach (super jump + Doubleflight from a standing place, twice)
    let (wall_top, margin) = (row("arena.mimic_tear.wall_top")[0], row("reach.margin")[0]);
    assert!(a.top >= wall_top + margin, "top {} >= {wall_top} + {margin}", a.top);
    let reach = row("reach.super_jump")[0] + row("reach.double_jump")[0];
    assert!(margin > reach && a.top - wall_top > reach, "the margin {margin} over KH3's reach {reach}");
    assert!(a.bottom < 0.0);
    // the doorways: the west one holds the fog gate (AEG099_002_9000, MSB (964.576, 1164.48)) and its passage region
    // 12022850 (centre (965.491, 1163.893), 11.6 m across); the north one the second fog gate (AEG099_002_9001)
    let along = |wall: usize, x: f32, z: f32| (Vec2::new(x, z) - a.walls[wall].0).dot(wall_frame(&a, wall).0);
    let w = a.gaps.iter().find(|g| g.name == "w_fog").unwrap();
    let fog = along(0, 964.576, 1164.48);
    let slab = along(0, 965.491, 1163.893);
    assert!(w.wall == 0 && w.s0 < fog && fog < w.s1 && w.s0 <= slab - 5.8 && slab + 5.8 <= w.s1, "the fog {fog}, its region {slab} +- 5.8 in {w:?}");
    let n = a.gaps.iter().find(|g| g.name == "n_door").unwrap();
    let fog2 = along(1, 1021.47, 1161.29);
    assert!(n.wall == 1 && n.s0 + 5.0 < fog2 && fog2 < n.s1 - 5.0, "the second fog {fog2} in {n:?}");
    // every window is on a wall, clear of the doorways
    for win in &a.windows {
        assert!(a.gaps.iter().all(|g| g.wall != win.wall || win.s1 < g.s0 || win.s0 > g.s1), "{win:?}");
    }
}

/// kh-partyhost's boss row of the Nokron Mimic Tear has the key the rows use.
fn kh_partyhost_key_is(key: &str) -> bool {
    #[cfg(feature = "party")]
    {
        kh_partyhost::mimicboss::boss_of_npc(526_100_965).is_some_and(|b| b.key == key && b.pool_entity == 12_020_860)
    }
    #[cfg(not(feature = "party"))]
    {
        key == "mimic_tear"
    }
}

#[test]
fn the_invisible_walls_stand_in_front_of_every_wall_closed_at_the_corners_open_at_the_doorways() {
    let a = nokron();
    let panels = panels_msb(&a);
    // 6 walls, two of them cut by a doorway (8 pieces), a lintel over each doorway, a return at each doorway's edge
    assert_eq!(panels.len(), 8 + 2 + 4);
    let (lo, hi) = (a.floor + a.bottom, a.floor + a.top);
    let full: Vec<&Panel> = panels.iter().filter(|p| p.lo == lo && p.hi == hi).collect();
    // every full piece lies on its wall's moved line: `inset` in front of the axis, parallel
    let mut along_walls = 0;
    for p in &panels {
        let on = (0..6).find(|&k| {
            let (_, n) = wall_frame(&a, k);
            let o = a.walls[k].0 + n * a.inset;
            (p.a - o).dot(n).abs() < 1e-3 && (p.b - o).dot(n).abs() < 1e-3
        });
        if on.is_some() {
            along_walls += 1;
        }
    }
    assert_eq!(along_walls, 8 + 2, "the pieces and the lintels on the moved lines");
    // the ring is closed: every corner is shared by two full pieces
    for k in 0..6 {
        let c = corner(&a, k);
        assert_eq!(full.iter().filter(|p| p.a.distance(c) < 1e-3 || p.b.distance(c) < 1e-3).count(), 2, "corner {k} at {c}");
    }
    // the doorways: nothing from the bottom to the gap's top, the lintel above; the returns reach through the axis
    for g in &a.gaps {
        let mid = on_wall(&a, g.wall, (g.s0 + g.s1) * 0.5, a.inset, 0.0);
        let block = panels.iter().filter(|p| {
            let (d, _) = wall_frame(&a, g.wall);
            let t = |q: Vec2| (q - Vec2::new(mid.x, mid.z)).dot(d);
            t(p.a).min(t(p.b)) < 0.0 && t(p.a).max(t(p.b)) > 0.0 && (Vec2::new(mid.x, mid.z) - p.a).perp_dot(p.b - p.a).abs() < 1e-2
        });
        let lows: Vec<f32> = block.map(|p| p.lo).collect();
        assert_eq!(lows, vec![a.floor + g.top], "{}: only the lintel over the doorway", g.name);
    }
}

#[test]
fn only_a_running_fight_with_the_leader_in_the_room_raises_them() {
    let a = nokron();
    let centre = Vec3::new(1001.327, a.floor, 1142.989);
    assert!(active(&a, true, centre));
    assert!(!active(&a, false, centre), "the fight off (not passed the fog, the win, the leader down)");
    // outside: beyond the fog gate, in the north corridor, far away, or below / above the room
    for p in [Vec3::new(960.0, a.floor, 1167.5), on_wall(&a, 1, 50.0, -6.0, 1.2), Vec3::new(0.0, 0.0, 0.0), centre - Vec3::Y * 10.0, centre + Vec3::Y * 80.0] {
        assert!(!active(&a, true, p), "{p}");
    }
    // in the fog's passage region (where the fog flag is set): in the room, in the doorway's gap
    assert!(active(&a, true, Vec3::new(965.491, -617.627, 1163.893)));
    // no fight told: no walls (the frame's word is taken once)
    fight("mimic_tear", true);
    assert_eq!(take_told(), Some(("mimic_tear", true)));
    assert_eq!(take_told(), None);
    // another fight's key has no rows: no walls
    assert!(arena_of("stray_mimic_tear").is_none() && arena_of("mimic_tear").is_some());
}

// ---------------------------------------------------------------------------------------------------- the casts

#[test]
fn a_sphere_cast_meets_a_panel_as_the_host_physics_reports_a_wall() {
    let p = Panel { a: Vec2::new(0.0, -500.0), b: Vec2::new(0.0, 500.0), lo: 0.0, hi: 1000.0, body: BODY_BASE + 3 };
    // head on: touches at 100 - 30 of 200
    let c = first_contact(&p, Vec3::new(-100.0, 0.0, 300.0), Vec3::new(200.0, 0.0, 0.0), 30.0).unwrap();
    assert!((c.fraction - 0.35).abs() < 1e-4 && c.point.distance(Vec3::new(0.0, 0.0, 300.0)) < 0.05 && c.normal.distance(Vec3::NEG_X) < 1e-3 && c.body == BODY_BASE + 3, "{c:?}");
    // from the other side: the other face
    let c = first_contact(&p, Vec3::new(100.0, 0.0, 300.0), Vec3::new(-200.0, 0.0, 0.0), 30.0).unwrap();
    assert!(c.normal.distance(Vec3::X) < 1e-3);
    // over the top, past the end, moving away, along it: nothing
    assert!(first_contact(&p, Vec3::new(-100.0, 0.0, 1040.0), Vec3::new(200.0, 0.0, 0.0), 30.0).is_none());
    assert!(first_contact(&p, Vec3::new(-100.0, 540.0, 300.0), Vec3::new(200.0, 0.0, 0.0), 30.0).is_none());
    assert!(first_contact(&p, Vec3::new(-100.0, 0.0, 300.0), Vec3::new(-200.0, 0.0, 0.0), 30.0).is_none());
    assert!(first_contact(&p, Vec3::new(-31.0, -100.0, 300.0), Vec3::new(0.0, 200.0, 0.0), 30.0).is_none());
    // the top edge: the separating direction
    let c = first_contact(&p, Vec3::new(-100.0, 0.0, 1020.0), Vec3::new(200.0, 0.0, 0.0), 30.0).unwrap();
    assert!(c.point.distance(Vec3::new(0.0, 0.0, 1000.0)) < 0.05 && c.normal.z > 0.5 && c.normal.x < 0.0, "{c:?}");
    // started inside: fraction 0, the way out
    let c = first_contact(&p, Vec3::new(-10.0, 0.0, 300.0), Vec3::new(50.0, 0.0, 0.0), 30.0).unwrap();
    assert!(c.fraction == 0.0 && c.normal.distance(Vec3::NEG_X) < 1e-3);
    // the walls have no traversal flags: no wall kick off them, no hanging from their top
    assert!(is_wall_body(BODY_BASE) && is_wall_body(BODY_BASE + 13) && !is_wall_body(1) && !is_wall_body(BODY_BASE - 1));
}

/// Every window, at every height of it from the sill to its top (or 30 m where it is open), a body moving straight out
/// (KH3's capsule, a sphere of every cast size the characters use) is stopped at the invisible wall; over the walls' top it
/// would pass.
#[test]
fn a_body_moving_out_through_any_window_at_any_height_is_stopped_in_front_of_it() {
    let a = nokron();
    let space = Space::new(&a);
    let room = Room::new(&a, &space, true);
    for win in &a.windows {
        let top = win.top.unwrap_or(30.0).min(30.0);
        for s in [win.s0 + 0.4, (win.s0 + win.s1) * 0.5, win.s1 - 0.4] {
            let mut up = win.sill;
            while up <= top {
                let from = space.kh(on_wall(&a, win.wall, s, 6.0, up));
                let to = space.kh(on_wall(&a, win.wall, s, -6.0, up));
                for (r, hh) in [(32.0, 89.0), (60.0, 89.0), (5.0, 5.0)] {
                    let hit = room.sweep_capsule(from, to, r, hh).unwrap_or_else(|| panic!("{} at s {s} up {up}: not stopped (r {r})", win.name));
                    let stop = from + (to - from) * hit.time;
                    assert!(beyond(&a, win.wall, &space, stop) < 0.0, "{} at s {s} up {up}: stopped beyond the invisible wall", win.name);
                    assert!(is_wall_body(hit.component));
                }
                up += 1.5;
            }
        }
    }
    // the top: under it stopped, over it not (the walls end 66 m up)
    let (under, over) = (a.top - 2.0, a.top + 2.0);
    for (up, stopped) in [(under, true), (over, false)] {
        let from = space.kh(on_wall(&a, 1, 30.0, 6.0, up));
        let to = space.kh(on_wall(&a, 1, 30.0, -6.0, up));
        assert_eq!(room.sweep_capsule(from, to, 32.0, 89.0).is_some(), stopped, "{up} m up");
    }
}

// ---------------------------------------------------------------------------------------------------- KH3's Sora

/// KH3's own Sora, level 99, at every window: from 4 m in front of it he runs, jumps (High Jump held), Doubleflights at the
/// top and then glides, or Air Slides, or (in battle) chains his aerial combo and an Aerial Dodge, with the stick toward the
/// window all the time. With the invisible walls he never gets past them; without them (the same script) he flies out
/// through the window, above its sill.
#[test]
fn sora_jumping_dashing_gliding_or_attacking_at_any_window_never_gets_past_the_invisible_wall() {
    let a = nokron();
    let space = Space::new(&a);
    let (walled, open) = (Room::new(&a, &space, true), Room::new(&a, &space, false));
    for win in &a.windows {
        let (_, n) = wall_frame(&a, win.wall);
        let yaw = space.yaw(-n);
        let feet = space.kh(on_wall(&a, win.wall, (win.s0 + win.s1) * 0.5, 4.0 + a.inset, 0.0));
        for way in WAYS {
            let held = run(&walled, feet, yaw, way);
            let worst = held.iter().map(|p| beyond(&a, win.wall, &space, *p)).fold(f32::MIN, f32::max);
            assert!(worst < -0.2, "{} {way:?}: got {worst:.2} m past the invisible wall", win.name);
            // he did come up to it, higher than the sill
            let near = held.iter().filter(|p| beyond(&a, win.wall, &space, **p) > -1.5);
            let high = near.map(|p| space.msb(*p).y - a.floor).fold(f32::MIN, f32::max);
            assert!(high > win.sill, "{} {way:?}: at the wall only {high:.2} m up", win.name);
            eprintln!("arena_walls: window {} {way:?}: held {:.2} m in front of the invisible wall, {high:.1} m up at it (sill {})", win.name, -worst, win.sill);
            // the same without the walls: out through the window, above the sill
            let free = run(&open, feet, yaw, way);
            let out = free.iter().find(|p| beyond(&a, win.wall, &space, **p) > a.inset + 1.0).unwrap_or_else(|| panic!("{} {way:?}: the script never reaches the window", win.name));
            assert!(space.msb(*out).y - a.floor > win.sill, "{} {way:?}: out under the sill", win.name);
        }
    }
}

/// The two doorways: KH3's Sora walks and runs straight out through the middle and near both jambs (0.6 m inside the real
/// jambs), and in again from outside, and diagonally (35 degrees) through them: every step the same as in a room without
/// invisible walls (they never touch him), and he gets 5 m out.
#[test]
fn sora_walks_through_both_doorways_out_and_in_straight_and_diagonally_untouched() {
    let a = nokron();
    let space = Space::new(&a);
    let (walled, open) = (Room::new(&a, &space, true), Room::new(&a, &space, false));
    // the real jambs (1.5 m up; the sheet's evidence): west s 21.03 / 31.14, north 45.68 / 55.51
    for (wall, jamb0, jamb1) in [(0usize, 21.03f32, 31.14f32), (1, 45.68, 55.51)] {
        let (_, n) = wall_frame(&a, wall);
        for s in [jamb0 + 0.6, (jamb0 + jamb1) * 0.5, jamb1 - 0.6] {
            for (into, toward, turn) in [(6.0f32, -n, 0.0f32), (-6.0, n, 0.0), (6.0, -n, 35.0), (6.0, -n, -35.0), (-6.0, n, 35.0)] {
                // a diagonal walk is aimed to cross the axis at s
                let dir = Vec2::from_angle(turn.to_radians()).rotate(toward);
                let start = on_wall(&a, wall, s, 0.0, 0.0) + Vec3::new(-dir.x, 0.0, -dir.y) * into.abs();
                let start = Vec3::new(start.x, a.floor, start.z);
                let yaw = space.yaw(dir);
                let walk = |room: &Room| {
                    let mut sora = level99_sora(space.kh(start), yaw);
                    (0..150).map(|_| sora.step(DT, &pad(yaw, &[], &[]), room).position).collect::<Vec<Vec3>>()
                };
                let (with, without) = (walk(&walled), walk(&open));
                let worst = with.iter().zip(&without).map(|(p, q)| p.distance(*q)).fold(0.0f32, f32::max);
                assert!(worst < 0.01, "wall {wall} s {s} from {into} at {turn} deg: the invisible walls moved him by {worst} cm");
                let end = space.msb(*with.last().unwrap());
                let past = (Vec2::new(end.x, end.z) - a.walls[wall].0).dot(n) * into.signum();
                assert!(past < -5.0, "wall {wall} s {s} from {into} at {turn} deg: only {past:.2} m through");
            }
        }
    }
}

/// The fog gate's passage: standing anywhere in its region 12022850 (where Elden Ring sets the fog flag and the walls go
/// up) that a body can stand in (the region's 11.6 m run past the doorway's real jambs into the wall), nothing of the walls
/// is touched: Sora's widest cast (32 cm) up to the real jambs, KH3's whole 60 cm capsule in the doorway's middle 9 m.
#[test]
fn the_fog_gates_passage_region_is_free_of_the_walls() {
    let a = nokron();
    let space = Space::new(&a);
    let room = Room::new(&a, &space, true);
    // the region: centre (965.491, -617.627, 1163.893), 2 x 11.6 m, yaw 29.9 deg (the slab across the doorway); the
    // doorway's real jambs (1.5 m up) at s 21.03 / 31.14 of the west wall
    let (c, yaw) = (Vec3::new(965.491, a.floor, 1163.893), 29.9f32.to_radians());
    let (_, n) = wall_frame(&a, 0);
    let along = |p: Vec3| (Vec2::new(p.x, p.z) - a.walls[0].0).dot(wall_frame(&a, 0).0);
    let mut checked = 0;
    for x in [-1.0f32, -0.5, 0.0, 0.5, 1.0] {
        for z in [-5.8f32, -4.0, -2.0, 0.0, 2.0, 4.0, 5.8] {
            let p = c + Vec3::new(x * yaw.cos() + z * yaw.sin(), 0.0, -x * yaw.sin() + z * yaw.cos());
            let s = along(p);
            for (r, (s0, s1)) in [(CAP, (21.03 + CAP / 100.0, 31.14 - CAP / 100.0)), (60.0, (21.6, 30.6))] {
                if s < s0 || s > s1 {
                    continue;
                }
                for up in [0.9f32, 1.8, 3.0] {
                    let centre = space.kh(p + Vec3::Y * up);
                    let touched: Vec<u32> = room.panels.iter().filter_map(|q| first_contact(q, centre, Vec3::X * 0.01, r)).map(|c| c.body).collect();
                    assert!(touched.is_empty(), "{p} (s {s:.2}, {:.2} m into the room) r {r}: {touched:?}", (Vec2::new(p.x, p.z) - a.walls[0].0).dot(n));
                    checked += 1;
                }
            }
        }
    }
    assert!(checked > 60, "{checked}");
}

/// The real host's world: the invisible walls carry no traversal flag (no wall kick, no dangle), every other body keeps the
/// mesh default.
#[cfg(windows)]
#[test]
fn the_hosts_world_gives_the_walls_no_traversal_flags() {
    let world = crate::world::ErWorld::new(Frame::at(Vec3::ZERO));
    assert_eq!(world.surface_flags(BODY_BASE + 2), 0);
    assert_eq!(world.surface_flags(7), surface::MESH_DEFAULT);
    assert!(world.arena.borrow().is_empty(), "no walls until a fight raises them");
}

#[test]
fn a_broken_sheet_gives_no_walls_rather_than_wrong_ones() {
    let open_ring = SHEET.replace("\"arena.mimic_tear.wall.6\",[1021.368", "\"arena.mimic_tear.wall.6\",[1031.368");
    assert!(parse(&open_ring).is_err(), "a ring that does not close");
    let off_wall = SHEET.replace("[2,43.96,57.16,18.4]", "[2,43.96,95.0,18.4]");
    assert!(parse(&off_wall).is_err(), "a doorway past its wall's end");
}
