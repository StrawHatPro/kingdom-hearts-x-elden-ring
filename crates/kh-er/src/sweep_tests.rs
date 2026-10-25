//! (IMPL_ER23A) Sora's own movement code run through the adapter over synthetic Elden Ring-like geometry: boxes whose
//! sphere casts answer AS THE GAME'S PHYSICS DOES (the touched point, the SEPARATING DIRECTION as "normal" on an edge,
//! fraction 0 for a start in overlap, nothing for a cast that leaves an overlap). What the user reported:
//! "collision issues going up stairs and with floor planks he will get stuck on them".

use glam::{Vec2, Vec3};
use kh_core::{Hit, Input, Pose, Sora, World, SORA_PARAMS};

use super::*;

/// Axis-aligned boxes (low corner, high corner); `true` = they are ONE body (one collision mesh, as a map piece of
/// Elden Ring is: `kh-core`s edge check ignores "the floor component", so it matters).
pub struct Boxes(pub Vec<(Vec3, Vec3)>, pub bool);

impl SphereCaster for Boxes {
    fn cast(&self, start: Vec3, delta: Vec3, radius: f32, out: &mut Vec<Contact>) {
        out.clear();
        let length = delta.length();
        if length < 1e-6 {
            return;
        }
        let direction = delta / length;
        for (k, &(lo, hi)) in self.0.iter().enumerate() {
            let body = if self.1 { 1 } else { k as u32 + 1 };
            let near = start.clamp(lo, hi);
            let distance = (start - near).length();
            if distance < radius - 1e-3 {
                // in overlap at the start: fraction 0 with the closest point, unless the cast leaves it
                let normal = if distance > 1e-5 { (start - near) / distance } else { Vec3::Z };
                if delta.dot(normal) < 1e-6 {
                    out.push(Contact { fraction: 0.0, point: near, normal, body });
                }
                continue;
            }
            // sphere tracing along the cast
            let mut t = 0.0f32;
            for _ in 0..40_000 {
                let p = start + direction * t;
                let q = p.clamp(lo, hi);
                let gap = (p - q).length() - radius;
                if gap < 1e-3 {
                    out.push(Contact { fraction: (t / length).clamp(0.0, 1.0), point: q, normal: (p - q).normalize_or_zero(), body });
                    break;
                }
                t += gap.max(0.02);
                if t > length {
                    break;
                }
            }
        }
    }
}

/// `kh_core::World` over a caster, as `world.rs` makes it (the slim capsule included).
pub struct Adapted<C: SphereCaster = Boxes> {
    pub boxes: C,
    pub opts: Options,
}

const SLIM: f32 = 32.0;

impl<C: SphereCaster> World for Adapted<C> {
    fn sweep_capsule(&self, start: Vec3, end: Vec3, radius: f32, half_height: f32) -> Option<Hit> {
        if radius <= 0.0 && half_height <= 0.0 {
            return line_first(&self.boxes, start, end, &self.opts);
        }
        sweep_first(&self.boxes, start, end, radius.min(SLIM), half_height, &self.opts)
    }
    fn line_trace(&self, start: Vec3, end: Vec3) -> Option<Hit> {
        line_first(&self.boxes, start, end, &self.opts)
    }
    fn sweep_capsule_all(&self, start: Vec3, end: Vec3, radius: f32, half_height: f32) -> Vec<Hit> {
        sweep_all(&self.boxes, start, end, radius.min(SLIM), half_height, &self.opts)
    }
}

fn slab(x0: f32, x1: f32, top: f32) -> (Vec3, Vec3) {
    (Vec3::new(x0, -400.0, top - 20.0), Vec3::new(x1, 400.0, top))
}

/// Boards 22 cm wide with 4 cm between them, their tops at uneven heights (0 .. 3 cm), from x = -200 on.
fn planks() -> Boxes {
    let tops = [0.0, 2.0, 1.0, 3.0, 0.5, 2.5, 0.0, 1.5];
    Boxes((0..80).map(|k| slab(-200.0 + 26.0 * k as f32, -200.0 + 26.0 * k as f32 + 22.0, tops[k % tops.len()])).collect(), true)
}

/// A landing, then twelve steps of 18 cm rise and 30 cm going.
fn stairs() -> Boxes {
    let mut boxes = vec![slab(-300.0, 200.0, 0.0)];
    for k in 0..12 {
        boxes.push((Vec3::new(200.0 + 30.0 * k as f32, -400.0, -20.0), Vec3::new(if k == 11 { 6000.0 } else { 230.0 + 30.0 * k as f32 }, 400.0, 18.0 * (k + 1) as f32)));
    }
    Boxes(boxes, true)
}

struct Run {
    /// (x, z, on the floor) per step.
    track: Vec<(f32, f32, bool)>,
}

impl Run {
    fn off_floor(&self, from: usize) -> usize {
        self.track[from..].iter().filter(|s| !s.2).count()
    }
    /// The longest stretch of steps off the floor in which he did not move (the user's "stuck in the air pose").
    fn hung(&self) -> usize {
        let (mut best, mut now) = (0, 0);
        for w in self.track.windows(2) {
            now = if !w[1].2 && (w[1].0 - w[0].0).abs() < 0.05 && (w[1].1 - w[0].1).abs() < 0.05 { now + 1 } else { 0 };
            best = best.max(now);
        }
        best
    }
}

fn run<C: SphereCaster>(world: &Adapted<C>, from: Vec3, stick: f32, steps: usize) -> Run {
    run2(world, from, Vec2::new(0.0, stick), steps)
}

/// The stick: x = to his right (KH3 +Y with the camera looking along +X), y = ahead (+X).
fn run2<C: SphereCaster>(world: &Adapted<C>, from: Vec3, stick: Vec2, steps: usize) -> Run {
    let mut sora = Sora::new(SORA_PARAMS, Pose { position: from + Vec3::Z * Sora::origin_height_on_floor(&SORA_PARAMS), yaw: 0.0 });
    let mut track = Vec::new();
    for _ in 0..steps {
        let out = sora.step(1.0 / 60.0, &Input { move_stick: stick, camera_yaw: 0.0, ..Input::default() }, world);
        track.push((out.position.x, out.position.z, out.on_floor));
    }
    Run { track }
}

#[test]
fn the_face_of_an_edge_is_the_tread_from_above_and_the_riser_from_the_side() {
    let boxes = Boxes(vec![(Vec3::new(0.0, -100.0, -50.0), Vec3::new(100.0, 100.0, 0.0))], true);
    // a sphere of 32 coming down 20 cm outside the box's edge: the physics says a slanted direction
    let (from, to) = (Vec3::new(-20.0, 0.0, 100.0), Vec3::new(-20.0, 0.0, 0.0));
    let plain = sweep_first(&boxes, from, to, 32.0, 32.0, &Options::PLAIN).unwrap();
    assert!(plain.impact_normal.z < 0.8 && plain.impact_normal.x < -0.6, "the separating direction: {plain:?}");
    let faced = sweep_first(&boxes, from, to, 32.0, 32.0, &Options::DEFAULT).unwrap();
    assert!((faced.impact_normal - Vec3::Z).length() < 1e-3, "the tread: {faced:?}");
    assert!((faced.normal - plain.normal).length() < 1e-4 && faced.time == plain.time, "only the surface normal changes");
    // the same edge met from the side (its centre 20 above the top): the riser
    let (from, to) = (Vec3::new(-100.0, 0.0, 20.0), Vec3::new(0.0, 0.0, 20.0));
    let faced = sweep_first(&boxes, from, to, 32.0, 32.0, &Options::DEFAULT).unwrap();
    assert!((faced.impact_normal - Vec3::NEG_X).length() < 1e-3, "the riser: {faced:?}");
    // a real slope keeps its normal; a flat face is not probed at all
    let flat = sweep_first(&boxes, Vec3::new(50.0, 0.0, 100.0), Vec3::new(50.0, 0.0, 0.0), 32.0, 32.0, &Options::DEFAULT).unwrap();
    assert!((flat.impact_normal - Vec3::Z).length() < 1e-4);
}

#[test]
fn a_floor_and_a_wall_overlapped_together_are_left_in_one_push() {
    // floor top at 0, a wall face at x = 0 (solid for x > 0): the sphere is 3 into the floor and 5 into the wall
    let boxes = Boxes(vec![(Vec3::new(-500.0, -500.0, -50.0), Vec3::new(500.0, 500.0, 0.0)), (Vec3::new(0.0, -500.0, 0.0), Vec3::new(50.0, 500.0, 300.0))], false);
    let at = Vec3::new(-27.0, 0.0, 29.0);
    let hit = sweep_first(&boxes, at, at - Vec3::Z * 5.0, 32.0, 32.0, &Options::DEFAULT).unwrap();
    assert!(hit.start_penetrating && hit.time == 0.0);
    let push = hit.normal * hit.penetration_depth;
    assert!((push - Vec3::new(-5.0, 0.0, 3.0)).length() < 0.2, "{push:?}");
    // the old answer: the deepest the move goes into = the floor alone; the wall is still overlapped after it
    let plain = sweep_first(&boxes, at, at - Vec3::Z * 5.0, 32.0, 32.0, &Options::PLAIN).unwrap();
    assert!((plain.normal - Vec3::Z).length() < 1e-3 && (plain.penetration_depth - 3.0).abs() < 0.01);
    // a move out of both is free
    assert!(sweep_first(&boxes, at, at + Vec3::new(-10.0, 0.0, 10.0), 32.0, 32.0, &Options::DEFAULT).is_none());
}

#[test]
fn a_crack_between_boards_is_closed_for_lines_and_thin_probes_only() {
    let boards = planks();
    // x = -200 + 22 .. -200 + 26 is a crack (boards with tops at 0 and 2); the line runs down its middle
    let x = -200.0 + 24.0;
    let (from, to) = (Vec3::new(x, 0.0, 30.0), Vec3::new(x, 0.0, -60.0));
    assert!(line_first(&boards, from, to, &Options::PLAIN).is_none(), "the line goes through the crack");
    let hit = line_first(&boards, from, to, &Options::DEFAULT).unwrap();
    assert!((hit.position.z - 2.0).abs() < 0.01 && (hit.impact_normal - Vec3::Z).length() < 1e-3, "the higher board's top: {hit:?}");
    assert!((hit.time - 28.0 / 90.0).abs() < 0.01);
    // the perch probe (radius 1)
    let probe = |opts: &Options| sweep_first(&boards, Vec3::new(x, 0.0, 30.0), Vec3::new(x, 0.0, -30.0), 1.0, 13.6, opts);
    assert!(probe(&Options::PLAIN).is_none());
    assert!((probe(&Options::DEFAULT).unwrap().position.z - 2.0).abs() < 0.01);
    // over a board the line is the line (no change), and a real drop (wider than the sphere) stays a drop
    let on = line_first(&boards, Vec3::new(-190.0, 0.0, 30.0), Vec3::new(-190.0, 0.0, -60.0), &Options::DEFAULT).unwrap();
    assert!((on.position.z - 0.0).abs() < 0.01 && (on.position.x + 190.0).abs() < 0.6);
    let pit = Boxes(vec![slab(-300.0, 0.0, 0.0), slab(40.0, 300.0, 0.0)], true);
    assert!(line_first(&pit, Vec3::new(20.0, 0.0, 30.0), Vec3::new(20.0, 0.0, -60.0), &Options::ER23A).is_none());
    // (IMPL_ER24) ... for the gap sphere; the foot rule holds him over a hole of 40 and leaves one of 100 a drop
    assert!(line_first(&pit, Vec3::new(20.0, 0.0, 30.0), Vec3::new(20.0, 0.0, -60.0), &Options::DEFAULT).is_some());
    let pit = Boxes(vec![slab(-300.0, 0.0, 0.0), slab(100.0, 300.0, 0.0)], true);
    assert!(line_first(&pit, Vec3::new(50.0, 0.0, 30.0), Vec3::new(50.0, 0.0, -60.0), &Options::DEFAULT).is_none());
}

/// THE USER'S STUCK PLACE: standing with his centre over a crack. Before: KH3's floor finding (lines, a 1 cm probe)
/// sees no ground under him: he is dropped into Falling although his body rests on the two boards. Now: he stands.
#[test]
fn standing_over_a_crack_between_boards() {
    let over = Vec3::new(-200.0 + 24.0 + 26.0 * 6.0, 0.0, 2.5);
    let before = run(&Adapted { boxes: planks(), opts: Options::PLAIN }, over, 0.0, 120);
    assert!(before.off_floor(0) >= 5, "the reproduction: he drops into the crack: off the floor {} steps", before.off_floor(0));
    let now = run(&Adapted { boxes: planks(), opts: Options::DEFAULT }, over, 0.0, 120);
    assert_eq!(now.off_floor(0), 0, "he stands on the boards");
}

#[test]
fn running_across_uneven_boards_with_cracks() {
    // before: Falling again and again on the way, pushed back and hung for several steps at a time (the air pose on the planks)
    let before = run(&Adapted { boxes: planks(), opts: Options::PLAIN }, Vec3::new(-150.0, 0.0, 3.0), 1.0, 120);
    assert!(before.off_floor(0) > 20 && before.track.windows(2).filter(|w| w[1].0 < w[0].0).count() >= 5, "the reproduction: {} steps off the floor", before.off_floor(0));
    let now = run(&Adapted { boxes: planks(), opts: Options::DEFAULT }, Vec3::new(-150.0, 0.0, 3.0), 1.0, 120);
    let last = now.track.last().unwrap();
    assert!(last.0 > 600.0, "he crossed the boards: x {}", last.0);
    assert_eq!(now.off_floor(0), 0, "never off the floor on the way");
    assert!(now.track.iter().all(|s| s.1 > -1.0 && s.1 < 6.0), "he neither sinks into a crack nor hops");
}

#[test]
fn a_flight_of_stairs_is_walked_up() {
    let now = run(&Adapted { boxes: stairs(), opts: Options::DEFAULT }, Vec3::new(0.0, 0.0, 0.0), 1.0, 200);
    let last = now.track.last().unwrap();
    assert!(last.1 > 18.0 * 12.0 - 1.0 && last.0 > 600.0, "at the top: x {} z {}", last.0, last.1);
    assert!(now.hung() < 3 && now.off_floor(10) < 12, "walked, not hopped: {} steps off the floor", now.off_floor(10));
    // and down again he comes off the last step onto the landing without hanging anywhere
    let slow = run(&Adapted { boxes: stairs(), opts: Options::DEFAULT }, Vec3::new(0.0, 0.0, 0.0), 0.6, 400);
    assert!(slow.track.last().unwrap().1 > 18.0 * 3.0 && slow.hung() < 3, "a slow walk climbs too: z {}", slow.track.last().unwrap().1);
}

#[test]
fn a_ledge_is_walked_off_without_hanging_on_its_edge() {
    // a platform ending at x = 300, the ground 250 below
    let world = Adapted { boxes: Boxes(vec![slab(-300.0, 300.0, 0.0), (Vec3::new(-300.0, -400.0, -300.0), Vec3::new(30000.0, 400.0, -250.0))], true), opts: Options::DEFAULT };
    for stick in [0.6, 1.0] {
        let now = run(&world, Vec3::new(150.0, 0.0, 0.0), stick, 400);
        let last = now.track.last().unwrap();
        assert!(last.2 && last.1 < -240.0 && last.0 > 300.0, "stick {stick}: he stands on the ground below: {last:?}");
        assert!(now.hung() < 6, "stick {stick}: hung on the edge for {} steps", now.hung());
    }
}

// ================================================================================================ (IMPL_ER24)
// The user, 2026-10-13, on the build with IMPL_ER23A's adapter: "going up to this door makes you fall thru the floor. Fix
// this and any other collision issues like Sora not being able to walk up these stairs without jumping".
// Each world is walked with the adapter as it was (`Options::ER23A`) and as it is (`Options::DEFAULT`).

/// Two collision meshes as Elden Ring has them: `main` answers his layer, `hi` only the floor layer.
pub struct Layers {
    pub main: Boxes,
    pub hi: Boxes,
}

impl SphereCaster for Layers {
    fn cast(&self, start: Vec3, delta: Vec3, radius: f32, out: &mut Vec<Contact>) {
        self.main.cast(start, delta, radius, out);
    }
    fn cast_extra(&self, start: Vec3, delta: Vec3, radius: f32, extra: Extra, out: &mut Vec<Contact>) {
        self.main.cast(start, delta, radius, out);
        if extra != Extra::None {
            let mut second = Vec::new();
            self.hi.cast(start, delta, radius, &mut second);
            for c in &mut second {
                c.body += 1000;
            }
            merge_extra(out, &second, start, delta, radius, extra);
        }
    }
}

/// Boxes and one slanted plane (solid below it), for ramps: exact sphere casts against the plane.
pub struct WithSlope {
    pub boxes: Boxes,
    /// A point of the plane and its unit normal.
    pub plane: (Vec3, Vec3),
}

impl SphereCaster for WithSlope {
    fn cast(&self, start: Vec3, delta: Vec3, radius: f32, out: &mut Vec<Contact>) {
        self.boxes.cast(start, delta, radius, out);
        let (point, normal) = self.plane;
        let height = (start - point).dot(normal);
        let approach = -delta.dot(normal);
        if height < radius {
            if height > -radius && approach > 0.0 {
                out.push(Contact { fraction: 0.0, point: start - normal * height, normal, body: 77 });
            }
        } else if approach > 0.0 {
            let fraction = (height - radius) / approach;
            if fraction <= 1.0 {
                out.push(Contact { fraction, point: start + delta * fraction - normal * radius, normal, body: 77 });
            }
        }
    }
}

const DEEP: f32 = -787.0;

fn deep_floor() -> (Vec3, Vec3) {
    (Vec3::new(-3000.0, -3000.0, DEEP - 50.0), Vec3::new(3000.0, 3000.0, DEEP))
}

fn wall(x: f32) -> (Vec3, Vec3) {
    (Vec3::new(x, -400.0, DEEP), Vec3::new(x + 60.0, 400.0, 400.0))
}

/// THE DOOR OF THE USER'S LOG (1285.9 s): a plank floor that ends `slot` cm before the door's wall, nothing his layer
/// sees in between, the next floor 7.87 m below.
fn door(slot: f32) -> Boxes {
    Boxes(vec![slab(-600.0, 0.0, 0.0), wall(slot), deep_floor()], false)
}

fn lowest(run: &Run) -> f32 {
    run.track.iter().map(|s| s.1).fold(f32::MAX, f32::min)
}

#[test]
fn the_door_whose_planks_end_before_the_wall() {
    // as it was: at walking speed he leaves the floor a few cm past the planks' end and falls 7.87 m (the user's trace:
    // floor lost about 10 cm past the edge, the wall met under the floor, landed 787 cm lower)
    let before = run(&Adapted { boxes: door(69.0), opts: Options::ER23A }, Vec3::new(-170.0, 0.0, 0.0), 0.6, 400);
    let last = before.track.last().unwrap();
    assert!(last.1 < DEEP + 10.0 && last.2, "the reproduction: he stands on the floor below: {last:?}");
    let lost = before.track.iter().find(|s| !s.2).unwrap();
    assert!(lost.0 > 0.0 && lost.0 < 25.0, "he lost the floor just past the planks' end: x {}", lost.0);
    // now (the foot rule alone, no second mesh): the slot is narrower than Elden Ring's own player: he walks to the door
    for stick in [0.6, 1.0] {
        let now = run(&Adapted { boxes: door(69.0), opts: Options::DEFAULT }, Vec3::new(-170.0, 0.0, 0.0), stick, if stick >= 1.0 { 220 } else { 700 });
        let last = now.track.last().unwrap();
        assert!(last.2 && last.0 > 30.0 && last.1 > -4.0, "stick {stick}: at the door, on the planks' level: {last:?}");
        assert!(lowest(&now) > -4.0 && now.off_floor(0) == 0, "stick {stick}: never off the floor, never sunk: lowest {}", lowest(&now));
    }
    // a slot Elden Ring's player would fall into as well (1.2 m) stays a drop
    let wide = run(&Adapted { boxes: door(120.0), opts: Options::DEFAULT }, Vec3::new(-170.0, 0.0, 0.0), 0.6, 500);
    assert!(wide.track.last().unwrap().1 < DEEP + 10.0, "a real gap: {:?}", wide.track.last());
}

#[test]
fn a_threshold_that_only_the_characters_mesh_has() {
    // the stone threshold (12 cm high, from 10 cm before the planks' end to the wall) exists in the second mesh only
    let world = |opts| Adapted { boxes: Layers { main: door(69.0), hi: Boxes(vec![(Vec3::new(-10.0, -400.0, -20.0), Vec3::new(69.0, 400.0, 12.0))], true) }, opts };
    let before = run(&world(Options::ER23A), Vec3::new(-170.0, 0.0, 0.0), 0.6, 400);
    assert!(before.track.last().unwrap().1 < DEEP + 10.0, "the reproduction: through the threshold he does not see");
    for stick in [0.6, 1.0] {
        let now = run(&world(Options::DEFAULT), Vec3::new(-170.0, 0.0, 0.0), stick, if stick >= 1.0 { 220 } else { 700 });
        let last = now.track.last().unwrap();
        assert!(last.2 && last.0 > 25.0 && (last.1 - 12.0).abs() < 3.5, "stick {stick}: he stands on the threshold at the door: {last:?}");
        assert!(lowest(&now) > -4.0, "stick {stick}: lowest {}", lowest(&now));
    }
    // a WALL of the second mesh is not taken (an invisible wall made for other characters): he walks on through it
    let fence = |opts| Adapted { boxes: Layers { main: Boxes(vec![slab(-600.0, 30000.0, 0.0)], true), hi: Boxes(vec![(Vec3::new(100.0, -400.0, 0.0), Vec3::new(110.0, 400.0, 300.0))], true) }, opts };
    let now = run(&fence(Options::DEFAULT), Vec3::new(0.0, 0.0, 0.0), 1.0, 120);
    assert!(now.track.last().unwrap().0 > 300.0 && now.off_floor(0) == 0, "{:?}", now.track.last());
}

/// A plank floor with a hole `width` wide across his way (from x = 0), a brick floor 3 m below.
fn hole(width: f32) -> Boxes {
    Boxes(vec![slab(-600.0, 0.0, 0.0), slab(width, 30000.0, 0.0), (Vec3::new(-3000.0, -3000.0, -350.0), Vec3::new(3000.0, 3000.0, -300.0))], true)
}

#[test]
fn holes_in_a_plank_floor_of_20_40_and_80_cm() {
    for (width, stick) in [(20.0, 0.6), (20.0, 1.0), (40.0, 0.6), (40.0, 1.0), (60.0, 0.6), (60.0, 1.0)] {
        let now = run(&Adapted { boxes: hole(width), opts: Options::DEFAULT }, Vec3::new(-150.0, 0.0, 0.0), stick, if stick >= 1.0 { 220 } else { 700 });
        let last = now.track.last().unwrap();
        assert!(last.2 && last.0 > width + 100.0 && last.1 > -4.0, "hole {width} stick {stick}: he crossed: {last:?}");
        assert!(lowest(&now) > -4.0 && now.off_floor(0) == 0, "hole {width} stick {stick}: lowest {} off the floor {}", lowest(&now), now.off_floor(0));
    }
    // as it was: over the 40 cm hole (his body cannot go down it) he is dropped into Falling and hangs there
    let before = run(&Adapted { boxes: hole(40.0), opts: Options::ER23A }, Vec3::new(-150.0, 0.0, 0.0), 0.6, 500);
    assert!(before.off_floor(0) >= 5, "the reproduction: off the floor {} steps at the 40 cm hole", before.off_floor(0));
    // 80 cm (the width of Elden Ring's own player) and more: he falls in, as before
    for width in [80.0, 120.0] {
        let now = run(&Adapted { boxes: hole(width), opts: Options::DEFAULT }, Vec3::new(-150.0, 0.0, 0.0), 0.6, 600);
        let last = now.track.last().unwrap();
        assert!(last.2 && last.1 < -290.0, "hole {width}: he stands on the floor below: {last:?}");
    }
}

/// Open-riser plank stairs: each tread a board 4 cm thick and `going` - 2 deep with air under it, `rise` apart.
fn plank_stairs(rise: f32, going: f32) -> Boxes {
    let mut boxes = vec![slab(-300.0, 200.0, 0.0)];
    for k in 0..12 {
        let top = rise * (k + 1) as f32;
        let x0 = 200.0 + going * k as f32;
        boxes.push((Vec3::new(x0, -400.0, top - 4.0), Vec3::new(if k == 11 { 30000.0 } else { x0 + going - 2.0 }, 400.0, top)));
    }
    boxes.push((Vec3::new(-3000.0, -3000.0, -350.0), Vec3::new(31000.0, 3000.0, -300.0)));
    Boxes(boxes, true)
}

#[test]
fn open_riser_plank_stairs_are_walked_up_and_down() {
    for (rise, going) in [(18.0, 30.0), (22.0, 28.0), (25.0, 25.0)] {
        for (name, opts) in [("ER23A", Options::ER23A), ("now", Options::DEFAULT)] {
            for stick in [0.6, 1.0] {
                let up = run(&Adapted { boxes: plank_stairs(rise, going), opts }, Vec3::new(0.0, 0.0, 0.0), stick, if stick >= 1.0 { 220 } else { 700 });
                let last = up.track.last().unwrap();
                assert!(last.2 && last.1 > rise * 12.0 - 1.0 && last.0 > 200.0 + going * 12.0, "{name} rise {rise} going {going} stick {stick}: at the top: {last:?}");
                assert!(up.hung() < 3, "{name} rise {rise} stick {stick}: hung {} steps", up.hung());
            }
        }
    }
}

#[test]
fn a_ramp_collider_of_40_degrees_is_walked_and_one_of_55_is_not() {
    let ramp = |degrees: f32, opts| {
        let a = degrees.to_radians();
        Adapted { boxes: WithSlope { boxes: Boxes(vec![slab(-600.0, 0.0, 0.0)], true), plane: (Vec3::ZERO, Vec3::new(-a.sin(), 0.0, a.cos())) }, opts }
    };
    for (name, opts) in [("ER23A", Options::ER23A), ("now", Options::DEFAULT)] {
        let up = run(&ramp(40.0, opts), Vec3::new(-100.0, 0.0, 0.0), 1.0, 240);
        let last = up.track.last().unwrap();
        assert!(last.2 && last.1 > 300.0, "{name}: up the 40 degree ramp (KH3 walks up to 49): {last:?}");
        assert!(up.off_floor(0) == 0, "{name}: off the floor {} steps on the ramp", up.off_floor(0));
        let steep = run(&ramp(55.0, opts), Vec3::new(-100.0, 0.0, 0.0), 1.0, 240);
        assert!(steep.track.last().unwrap().1 < 60.0, "{name}: 55 degrees is a wall for KH3's movement: {:?}", steep.track.last());
    }
}

/// A ramp of boards: each 22 wide and 4 thick with 4 cm of air between and under them, `rise` higher than the last.
fn plank_ramp(rise: f32) -> Boxes {
    let mut boxes = vec![slab(-300.0, 0.0, 0.0)];
    for k in 0..40 {
        let top = rise * (k + 1) as f32;
        boxes.push((Vec3::new(4.0 + 26.0 * k as f32, -400.0, top - 4.0), Vec3::new(if k == 39 { 30000.0 } else { 26.0 + 26.0 * k as f32 }, 400.0, top)));
    }
    boxes.push((Vec3::new(-3000.0, -3000.0, -350.0), Vec3::new(31000.0, 3000.0, -300.0)));
    Boxes(boxes, true)
}

#[test]
fn a_ramp_of_planks_with_gaps() {
    for rise in [4.0, 10.0] {
        for stick in [0.6, 1.0] {
            let now = run(&Adapted { boxes: plank_ramp(rise), opts: Options::DEFAULT }, Vec3::new(-100.0, 0.0, 0.0), stick, if stick >= 1.0 { 220 } else { 700 });
            let last = now.track.last().unwrap();
            assert!(last.2 && last.1 > rise * 40.0 - 1.0 && last.0 > 1100.0, "rise {rise} stick {stick}: at the top: {last:?}");
            assert!(now.hung() < 3 && now.off_floor(0) < 6, "rise {rise} stick {stick}: hung {} off the floor {}", now.hung(), now.off_floor(0));
        }
    }
}

#[test]
fn a_threshold_stone_before_a_door_with_a_hole_in_the_planks_beside_it() {
    // planks, a hole of 25 cm right before the stone, the stone 13 cm high up to the wall
    let boxes = || Boxes(vec![slab(-600.0, 0.0, 0.0), (Vec3::new(25.0, -400.0, -20.0), Vec3::new(120.0, 400.0, 13.0)), wall(120.0), deep_floor()], false);
    for stick in [0.6, 1.0] {
        let now = run(&Adapted { boxes: boxes(), opts: Options::DEFAULT }, Vec3::new(-170.0, 0.0, 0.0), stick, if stick >= 1.0 { 220 } else { 700 });
        let last = now.track.last().unwrap();
        assert!(last.2 && last.0 > 60.0 && (last.1 - 13.0).abs() < 3.5, "stick {stick}: on the stone at the door: {last:?}");
        assert!(lowest(&now) > -4.0 && now.hung() < 3, "stick {stick}: lowest {} hung {}", lowest(&now), now.hung());
    }
}

#[test]
fn loose_boards_lying_on_a_floor_are_walked_over_at_any_angle() {
    // boards 3 .. 8 cm high lying on the planks, crossed straight and at slants (the stick turned 20 and 40 degrees)
    let mut boxes = vec![(Vec3::new(-600.0, -30000.0, -20.0), Vec3::new(30000.0, 30000.0, 0.0))];
    for (k, high) in [3.0f32, 8.0, 5.0, 6.0, 4.0, 7.0].into_iter().enumerate() {
        let x = 30.0 + 70.0 * k as f32;
        boxes.push((Vec3::new(x, -30000.0, 0.0), Vec3::new(x + 18.0, 30000.0, high)));
    }
    // two that overlap (one lying on the other)
    boxes.push((Vec3::new(470.0, -30000.0, 0.0), Vec3::new(500.0, 30000.0, 5.0)));
    boxes.push((Vec3::new(485.0, -30000.0, 5.0), Vec3::new(520.0, 30000.0, 9.0)));
    for (name, opts) in [("ER23A", Options::ER23A), ("now", Options::DEFAULT)] {
        for degrees in [0.0f32, 20.0, 40.0] {
            for speed in [0.6, 1.0] {
                let a = degrees.to_radians();
                let now = run2(&Adapted { boxes: Boxes(boxes.clone(), false), opts }, Vec3::new(-100.0, 0.0, 0.0), Vec2::new(a.sin(), a.cos()) * speed, if speed >= 1.0 { 220 } else { 700 });
                let last = now.track.last().unwrap();
                assert!(last.2 && last.0 > 600.0 && last.1.abs() < 1.0, "{name} {degrees} degrees speed {speed}: he crossed the boards: {last:?}");
                assert!(now.hung() < 3 && now.off_floor(0) < 10, "{name} {degrees} degrees speed {speed}: hung {} off the floor {}", now.hung(), now.off_floor(0));
            }
        }
    }
}

#[test]
fn a_line_that_starts_a_little_inside_a_thin_board_still_finds_it() {
    // a board 4 cm thick, air under it; the line starts 1.5 cm under its top (he sank into it)
    let board = Boxes(vec![(Vec3::new(-100.0, -100.0, -4.0), Vec3::new(100.0, 100.0, 0.0))], true);
    let (from, to) = (Vec3::new(0.0, 0.0, -1.5), Vec3::new(0.0, 0.0, -60.0));
    assert!(line_first(&board, from, to, &Options::ER23A).is_none_or(|h| h.time == 0.0), "as it was: nothing under him (a hit at time 0 is none for KH3's floor line)");
    let hit = line_first(&board, from, to, &Options::DEFAULT).unwrap();
    assert!(hit.time > 0.0 && hit.time < 0.01 && hit.position.z.abs() < 0.01 && hit.impact_normal.z > 0.99, "{hit:?}");
    // a line from above is as it was
    let hit = line_first(&board, Vec3::new(0.0, 0.0, 30.0), Vec3::new(0.0, 0.0, -60.0), &Options::DEFAULT).unwrap();
    assert!((hit.time - 30.0 / 90.0).abs() < 0.01);
}

#[test]
fn a_push_out_of_an_overlap_does_not_go_into_what_is_behind_him() {
    // 3 cm into the floor, a ceiling 1 cm above his head
    let boxes = Boxes(vec![(Vec3::new(-500.0, -500.0, -50.0), Vec3::new(500.0, 500.0, 0.0)), (Vec3::new(-500.0, -500.0, 62.0), Vec3::new(500.0, 500.0, 100.0))], false);
    let at = Vec3::new(0.0, 0.0, 29.0);
    let before = sweep_first(&boxes, at, at - Vec3::Z * 5.0, 32.0, 32.0, &Options::ER23A).unwrap();
    assert!(before.start_penetrating && (before.penetration_depth - 3.0).abs() < 0.05, "as it was: 3 cm up, into the ceiling: {before:?}");
    // (the rule is off by default since a lift rose through him in the game: `w.clamp = 1`)
    assert!(!Options::DEFAULT.clamp);
    let clamp = Options { clamp: true, ..Options::DEFAULT };
    let now = sweep_first(&boxes, at, at - Vec3::Z * 5.0, 32.0, 32.0, &clamp).unwrap();
    assert!(now.start_penetrating && now.penetration_depth < 0.8, "cut short under the ceiling: {now:?}");
    // with room above it is the whole push
    let open = Boxes(vec![boxes.0[0]], false);
    let now = sweep_first(&open, at, at - Vec3::Z * 5.0, 32.0, 32.0, &clamp).unwrap();
    assert!((now.penetration_depth - 3.0).abs() < 0.05);
}

#[test]
fn the_foot_rule_holds_over_holes_and_slots_and_not_at_a_ledge() {
    let opts = Options::DEFAULT;
    let at = |x: f32| Vec3::new(x, 0.0, 2.0);
    // a ledge: nothing on the far side
    let ledge = Boxes(vec![slab(-600.0, 0.0, 0.0), deep_floor()], true);
    assert!(foot_support(&ledge, at(10.0), 59.0, &opts).is_none());
    // a hole of 40, 60, 76: held at the planks' height; of 80 and 100: not
    for (width, held) in [(40.0, true), (60.0, true), (76.0, true), (80.0, false), (100.0, false)] {
        for x in [5.0, 15.0, 30.0] {
            assert_eq!(foot_support(&hole(width), at(x), 59.0, &opts).map(|s| s.0.round()), held.then_some(0.0), "hole {width} at {x}");
        }
    }
    // the slot before a wall: held; and nothing is said where there is plain ground under the axis' sphere
    assert!(foot_support(&door(69.0), at(10.0), 59.0, &opts).is_some() && foot_support(&door(69.0), at(30.0), 59.0, &opts).is_some());
    assert!(foot_support(&door(120.0), at(10.0), 59.0, &opts).is_none());
    assert_eq!(foot_support(&ledge, at(-200.0), 59.0, &opts).map(|s| s.0), Some(0.0));
}

/// WHAT MAY BE THE USER'S STAIRS (not determined: his log has no contacts): the mesh his layer sees lacks two treads
/// (a rise of 54 cm, more than KH3's step of 47); the characters' mesh has a smooth ramp over the whole flight (here
/// fine steps of 3 cm). As it was: he stops at the high riser and needs a jump. With the floor layer: he walks up.
#[test]
fn stairs_with_a_missing_tread_and_a_ramp_in_the_characters_mesh() {
    let mut main = vec![slab(-300.0, 200.0, 0.0)];
    for k in 0..12 {
        // (treads 4 and 5 are missing: the boards of tread 3 go on level to the riser of tread 6)
        let top = 18.0 * (if (4..6).contains(&k) { 4 } else { k + 1 }) as f32;
        main.push((Vec3::new(200.0 + 30.0 * k as f32, -400.0, -20.0), Vec3::new(if k == 11 { 30000.0 } else { 230.0 + 30.0 * k as f32 }, 400.0, top)));
    }
    let ramp: Vec<_> = (0..72).map(|k| (Vec3::new(200.0 + 5.0 * k as f32, -400.0, -20.0), Vec3::new(205.0 + 5.0 * k as f32, 400.0, 3.0 * (k + 1) as f32))).collect();
    let world = |opts| Adapted { boxes: Layers { main: Boxes(main.clone(), true), hi: Boxes(ramp.clone(), true) }, opts };
    let before = run(&world(Options::ER23A), Vec3::new(0.0, 0.0, 0.0), 1.0, 220);
    let last = before.track.last().unwrap();
    assert!(last.0 < 380.0 && last.1 < 75.0 && last.2, "the reproduction: stopped before the high riser: {last:?}");
    for stick in [0.6, 1.0] {
        let now = run(&world(Options::DEFAULT), Vec3::new(0.0, 0.0, 0.0), stick, if stick >= 1.0 { 220 } else { 700 });
        let last = now.track.last().unwrap();
        assert!(last.2 && last.1 > 18.0 * 12.0 - 1.0 && last.0 > 600.0, "stick {stick}: at the top: {last:?}");
        assert!(now.hung() < 3, "stick {stick}: hung {} steps", now.hung());
    }
}
