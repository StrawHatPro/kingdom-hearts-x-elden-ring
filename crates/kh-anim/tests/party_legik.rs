//! (party A2) The friends' leg IK: Goofy's TwoBoneIKMultiped with the stock solver (0x14054FC10) and Donald's two
//! CollTwoBoneIK nodes (0x140536200, BOX sweeps), on flat ground, a step and a slope; plus the pieces (stock two-bone
//! solve, swing / twist, the box sweep).
//!
//! `cargo test --release -p kh-anim --test party_legik -- --nocapture`

use std::path::PathBuf;

use glam::{Quat, Vec3};
use kh_anim::legik::{remove_twist, solve_two_bone_positions, to_swing_twist};
use kh_anim::{sweep_obb, Animator, BodySlot, Frame, IkShape, IkWorld, NoIkWorld, Obb, Pack, Settings, ShapeHit, SweepHit, Trs};

fn pack_dir(npc: &str) -> Option<PathBuf> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_pack/party").join(npc);
    if dir.join("data/legik.json").is_file() {
        Some(dir)
    } else {
        println!("no {} (node tools/party_pack.js): skipped", dir.join("data/legik.json").display());
        None
    }
}

#[test]
fn stock_two_bone_solve_reaches_and_bends_toward_the_target() {
    // upper 40, lower 40, effector 60 away: the joint lies off the line, toward the joint target
    let (root, effector, target) = (Vec3::ZERO, Vec3::new(0.0, 0.0, -60.0), Vec3::new(50.0, 0.0, -30.0));
    let (joint, end) = solve_two_bone_positions(root, effector, target, 40.0, 40.0, false, 1.0, 1.2);
    assert!((end - effector).length() < 1.0e-4, "end {end}");
    assert!(((joint - root).length() - 40.0).abs() < 1.0e-3 && ((end - joint).length() - 40.0).abs() < 1.0e-3, "lengths kept: {joint}");
    assert!(joint.x > 20.0 && joint.y.abs() < 1.0e-4, "bent toward +X: {joint}");
    // out of reach: straight along the line, the end at full length
    let (joint, end) = solve_two_bone_positions(root, Vec3::new(0.0, 0.0, -100.0), target, 40.0, 40.0, false, 1.0, 1.2);
    assert!((end - Vec3::new(0.0, 0.0, -80.0)).length() < 1.0e-4 && (joint - Vec3::new(0.0, 0.0, -40.0)).length() < 1.0e-4);
}

#[test]
fn swing_twist_and_remove_twist() {
    let q = Quat::from_rotation_z(0.4) * Quat::from_rotation_x(0.7);
    let (swing, twist) = to_swing_twist(q, Vec3::X);
    assert!((swing * twist).dot(q).abs() > 0.99999);
    assert!((twist * Vec3::X - Vec3::X).length() < 1.0e-5, "the twist turns about X only");
    // RemoveTwist keeps the new swing and the ORIGINAL twist
    let parent = Trs::IDENTITY;
    let original = Trs { rotation: Quat::from_rotation_x(0.2), ..Trs::IDENTITY };
    let mut bone = Trs { rotation: Quat::from_rotation_y(0.5) * Quat::from_rotation_x(1.1), ..Trs::IDENTITY };
    remove_twist(&parent, &mut bone, &original, Vec3::X);
    let (_, t) = to_swing_twist(bone.rotation, Vec3::X);
    let (_, t0) = to_swing_twist(original.rotation, Vec3::X);
    assert!(t.dot(t0).abs() > 0.99999, "twist {t:?} vs {t0:?}");
}

#[test]
fn box_sweep_hits_lands_and_reports_initial_depth() {
    let ground = Obb::new(Vec3::new(0.0, 0.0, -50.0), Quat::IDENTITY, Vec3::new(500.0, 500.0, 50.0));
    // a 2 cm half-height box from z 10 down to z -10: touches when its bottom reaches 0 (centre at 2: time 0.4)
    let mover = Obb::new(Vec3::new(0.0, 0.0, 10.0), Quat::IDENTITY, Vec3::new(5.0, 3.0, 2.0));
    let (time, normal, depth) = sweep_obb(&mover, Vec3::new(0.0, 0.0, -20.0), &ground).unwrap();
    assert!((time - 0.4).abs() < 1.0e-5 && (normal - Vec3::Z).length() < 1.0e-5 && depth == 0.0, "{time} {normal} {depth}");
    // starting 1.5 cm inside: time 0, depth 1.5 along +Z
    let inside = Obb::new(Vec3::new(0.0, 0.0, 0.5), Quat::IDENTITY, Vec3::new(5.0, 3.0, 2.0));
    let (time, normal, depth) = sweep_obb(&inside, Vec3::new(0.0, 0.0, -5.0), &ground).unwrap();
    assert!(time == 0.0 && (normal - Vec3::Z).length() < 1.0e-5 && (depth - 1.5).abs() < 1.0e-4, "{time} {normal} {depth}");
    // passing beside: no hit
    assert!(sweep_obb(&Obb::new(Vec3::new(0.0, 900.0, 10.0), Quat::IDENTITY, Vec3::ONE), Vec3::new(0.0, 0.0, -20.0), &ground).is_none());
    // a rotated box: the corner leads
    let tilted = Obb::new(Vec3::new(0.0, 0.0, 10.0), Quat::from_rotation_y(45f32.to_radians()), Vec3::splat(2.0));
    let (time, _, _) = sweep_obb(&tilted, Vec3::new(0.0, 0.0, -20.0), &ground).unwrap();
    let expect = (10.0 - 2.0 * 2f32.sqrt()) / 20.0;
    assert!((time - expect).abs() < 1.0e-4, "{time} vs {expect}");
}

/// The ground of a test: flat, a step (raised for x > edge) or a slope (rising toward +X). Spheres are swept against
/// the analytic planes, boxes against slab boxes of the same surfaces.
#[derive(Clone, Copy)]
enum Ground {
    Flat,
    Step { edge: f32, high: f32 },
    Slope { degrees: f32 },
}

impl Ground {
    fn height(&self, p: Vec3) -> f32 {
        match *self {
            Ground::Flat => 0.0,
            Ground::Step { edge, high } => if p.x > edge { high } else { 0.0 },
            Ground::Slope { degrees } => p.x * degrees.to_radians().tan(),
        }
    }

    fn plane(&self, p: Vec3) -> (Vec3, Vec3) {
        match *self {
            Ground::Slope { degrees } => (Vec3::ZERO, Quat::from_rotation_y(-degrees.to_radians()) * Vec3::Z),
            _ => (Vec3::new(0.0, 0.0, self.height(p)), Vec3::Z),
        }
    }

    fn slabs(&self) -> Vec<Obb> {
        let slab = |rotation: Quat, top: Vec3| Obb::new(top + rotation * Vec3::new(0.0, 0.0, -100.0), rotation, Vec3::new(1000.0, 1000.0, 100.0));
        match *self {
            Ground::Flat => vec![slab(Quat::IDENTITY, Vec3::ZERO)],
            Ground::Step { edge, high } => vec![slab(Quat::IDENTITY, Vec3::ZERO), Obb::new(Vec3::new(edge + 500.0, 0.0, high - 100.0), Quat::IDENTITY, Vec3::new(500.0, 1000.0, 100.0))],
            Ground::Slope { degrees } => vec![slab(Quat::from_rotation_y(-degrees.to_radians()), Vec3::ZERO)],
        }
    }
}

impl IkWorld for Ground {
    fn sweep_sphere(&self, start: Vec3, end: Vec3, radius: f32) -> Option<SweepHit> {
        let (point, normal) = self.plane(start);
        let (d0, d1) = ((start - point).dot(normal), (end - point).dot(normal));
        if d0 < radius {
            return Some(SweepHit { time: 0.0, impact_point: start - normal * d0, impact_normal: normal, start_penetrating: true });
        }
        if d1 > radius {
            return None;
        }
        let time = (d0 - radius) / (d0 - d1);
        let centre = start + (end - start) * time;
        Some(SweepHit { time, impact_point: centre - normal * radius, impact_normal: normal, start_penetrating: false })
    }

    fn sweep_shape(&self, start: Vec3, end: Vec3, shape: IkShape, rotation: Quat) -> Option<ShapeHit> {
        let IkShape::Box(half) = shape else { return self.sweep_sphere(start, end, match shape { IkShape::Sphere(r) => r, _ => 0.0 }).map(|hit| ShapeHit { hit, penetration_depth: 0.0 }) };
        let mover = Obb::new(start, rotation, half);
        let mut best: Option<(f32, Vec3, f32)> = None;
        for s in self.slabs() {
            if let Some(f) = sweep_obb(&mover, end - start, &s) {
                if best.is_none_or(|b| f.2 > b.2 || (f.2 == 0.0 && b.2 == 0.0 && f.0 < b.0)) {
                    best = Some(f);
                }
            }
        }
        best.map(|(time, normal, depth)| ShapeHit { hit: SweepHit { time, impact_point: start + (end - start) * time, impact_normal: normal, start_penetrating: depth > 0.0 }, penetration_depth: depth })
    }
}

/// IdleN (0001, IK window over the whole clip) for 120 steps at 60 Hz; the last pose in the world (component = world:
/// the actor at the origin, mesh facing +Y).
fn idle(npc: &str, leg_ik: bool, world: &dyn IkWorld) -> Option<(Animator, kh_anim::Pose)> {
    let dir = pack_dir(npc)?;
    let mut animator = Animator::new(Pack::load(&dir).unwrap(), Settings { leg_ik, bonamik: false, ..Settings::default() }).unwrap();
    let sequences = ["0001"];
    let (mut time, mut last) = (0.0f32, None);
    for step in 0..120 {
        let dt = 1.0 / 60.0;
        let from = if step == 0 { -1.0 } else { time };
        time += dt;
        let body = BodySlot { samples: [("0001", 1.0), ("0001", 0.0)], blend_with: None, notify_sequences: &sequences, time, weight: 1.0, active: true, notify_window: (from, time) };
        last = Some(animator.step(&Frame { dt, body, component_to_world: Trs::IDENTITY }, world).clone());
    }
    Some((animator, last.unwrap()))
}

fn feet(a: &Animator) -> [usize; 2] {
    [a.skeleton().index_of("L_ashi1").unwrap(), a.skeleton().index_of("R_ashi1").unwrap()]
}

#[test]
fn goofy_feet_plant_on_a_step_and_a_slope() {
    let Some((a, rest)) = idle("n_ex002", false, &NoIkWorld) else { return };
    let [l, r] = feet(&a);
    let (lp, rp) = (rest.component[l].translation, rest.component[r].translation);
    println!("Goofy IdleN feet without IK: L {lp:.1}  R {rp:.1}");
    // flat ground at the actor's origin: nothing to do (delta 0 on a flat floor, 4.2)
    let (a2, flat) = idle("n_ex002", true, &Ground::Flat).unwrap();
    println!("Goofy flat: kind {}, alpha {:.2}, report {:?}", a2.leg_ik_kind(), a2.leg_alpha().value, a2.leg_ik_report());
    assert_eq!(a2.leg_ik_kind(), "multiped (stock solver)", "{:?}", a2.warnings);
    assert!(a2.leg_alpha().value > 0.9, "IdleN's IK window: alpha {}", a2.leg_alpha().value);
    for f in [l, r] {
        assert!((flat.component[f].translation - rest.component[f].translation).length() < 1.0, "flat: foot {f} moved");
    }
    // a 6 cm step under the foot on the +X side
    let (hi, lo) = if lp.x > rp.x { (l, r) } else { (r, l) };
    let edge = (lp.x + rp.x) * 0.5;
    let step = Ground::Step { edge, high: 6.0 };
    let (a3, on_step) = idle("n_ex002", true, &step).unwrap();
    let raise = on_step.component[hi].translation.z - rest.component[hi].translation.z;
    let other = on_step.component[lo].translation.z - rest.component[lo].translation.z;
    println!("Goofy step 6 cm: raised foot +{raise:.2} cm, other {other:+.2} cm; report {:?}", a3.leg_ik_report());
    assert!(raise > 4.0 && raise < 7.0, "the foot on the step goes up: {raise}");
    assert!(other.abs() < 1.0, "the other foot stays: {other}");
    // a 12 degree slope rising toward +X: each foot follows the ground under it; the hips drop for the low one
    let slope = Ground::Slope { degrees: 12.0 };
    let (a4, on_slope) = idle("n_ex002", true, &slope).unwrap();
    for f in [l, r] {
        let (p0, p1) = (rest.component[f].translation, on_slope.component[f].translation);
        let want = p0.z + slope.height(p0);
        println!("Goofy slope 12: foot {f}: {:.2} -> {:.2} (ground under it {:.2}, wanted ~{want:.2})", p0.z, p1.z, slope.height(p0));
        assert!((p1.z - want).abs() < 2.5, "foot {f} on the slope: {} vs {want}", p1.z);
    }
    println!("Goofy slope report {:?}", a4.leg_ik_report());
    assert!(a4.leg_ik_report().hips_drop > 0.0, "the hips drop toward the lower foot");
}

#[test]
fn donald_feet_follow_the_ground_with_box_sweeps() {
    let Some((a, rest)) = idle("n_ex001", false, &NoIkWorld) else { return };
    let [l, r] = feet(&a);
    let (lp, rp) = (rest.component[l].translation, rest.component[r].translation);
    println!("Donald IdleN feet without IK: L {lp:.2}  R {rp:.2}");
    let (a2, flat) = idle("n_ex001", true, &Ground::Flat).unwrap();
    assert_eq!(a2.leg_ik_kind(), "coll x2", "{:?}", a2.warnings);
    println!("Donald flat: alpha {:.2}, {:?}", a2.leg_alpha().value, a2.coll_ik_reports());
    for f in [l, r] {
        let d = flat.component[f].translation - rest.component[f].translation;
        println!("  flat: foot {f} moved {d:.2}");
        assert!(d.length() < 3.0);
    }
    // The effector does not depend on the hit time (d = Eo + up (PenetrationDepth + 0.01), native_D_legik 5): a step
    // the sweep meets on its way gives the same -0.99 cm as the flat floor ...
    let (hi, lo) = if lp.x > rp.x { (l, r) } else { (r, l) };
    let (a3, on_step) = idle("n_ex001", true, &Ground::Step { edge: (lp.x + rp.x) * 0.5, high: 3.0 }).unwrap();
    let raise = on_step.component[hi].translation.z - flat.component[hi].translation.z;
    println!("Donald step 3 cm: foot on the step {raise:+.2} cm against flat; {:?}", a3.coll_ik_reports());
    assert!(raise.abs() < 0.05, "met on the way: as flat: {raise}");
    // ... while ground the box already overlaps at the sweep's start lifts the foot by the overlap depth
    let (a5, on_high) = idle("n_ex001", true, &Ground::Step { edge: (lp.x + rp.x) * 0.5, high: 10.0 }).unwrap();
    let raise = on_high.component[hi].translation.z - flat.component[hi].translation.z;
    let other = on_high.component[lo].translation.z - flat.component[lo].translation.z;
    let pen = a5.coll_ik_reports().iter().map(|r| r.penetration_depth).fold(0.0f32, f32::max);
    println!("Donald step 10 cm: foot on the step {raise:+.2} cm against flat (depth {pen:.2}), other {other:+.2}; {:?}", a5.coll_ik_reports());
    assert!(pen > 0.0 && (raise - pen).abs() < 0.3, "pushed up by the depth: {raise} vs {pen}");
    assert!(other.abs() < 0.5);
    // a 20 degree slope rising toward +X: the uphill foot's box overlaps the slope: lifted and turned to the normal
    // (limit 30 degrees); the downhill foot's short sweep (4 cm above to 1 cm below) finds nothing: animated pose
    let (a4, on_slope) = idle("n_ex001", true, &Ground::Slope { degrees: 20.0 }).unwrap();
    for (f, uphill) in [(hi, true), (lo, false)] {
        let turn = 2.0 * rest.component[f].rotation.dot(on_slope.component[f].rotation).abs().min(1.0).acos().to_degrees();
        let lift = on_slope.component[f].translation.z - rest.component[f].translation.z;
        println!("Donald slope 20: {} foot {f} turned {turn:.1} deg, lifted {lift:+.2} cm", if uphill { "uphill" } else { "downhill" });
        if uphill {
            assert!((turn - 20.0).abs() < 1.0 && lift > 1.0, "uphill foot: {turn} {lift}");
        } else {
            assert!(turn < 0.5 && lift.abs() < 0.05, "downhill foot: {turn} {lift}");
        }
    }
    println!("Donald slope {:?}", a4.coll_ik_reports());
}
