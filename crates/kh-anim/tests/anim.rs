//! kh-anim against docs/sora-animation-spec.md Part II ("spec N") and the change list
//! docs/sora-animation-spec-changes.md ("change A1" ...), on the real pack (build/sora_pack).
//! Without a pack on disk the tests that need it say so and pass.

use std::path::PathBuf;

use glam::{Quat, Vec3};
use kh_anim::blend::{key_rotation, key_scale, slot, weighted};
use kh_anim::kine::KineDriver;
use kh_anim::legik::{maya_solve, IkWorld, SweepHit, ALPHA_CAP};
use kh_anim::pose::component_space;
use kh_anim::{Animator, BodySlot, Frame, NoIkWorld, Pack, Pose, Settings, Trs};

const DT: f32 = 1.0 / 60.0;

fn pack() -> Option<Pack> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_pack");
    if !dir.join("manifest.json").is_file() {
        eprintln!("SKIPPED: no pack at {}", dir.display());
        return None;
    }
    Some(Pack::load(&dir).expect("the pack loads"))
}

fn angle_deg(a: Quat, b: Quat) -> f32 {
    2.0 * a.dot(b).abs().min(1.0).acos().to_degrees()
}

/// The mesh component in the world for an actor at `origin` with `yaw` degrees.
fn c2w(origin: Vec3, yaw: f32) -> Trs {
    Trs { translation: origin, rotation: Quat::from_rotation_z((yaw - 90.0).to_radians()), scale: Vec3::ONE }
}

/// Plays sequences as the simulation would report them: clock, weight, notify windows.
struct Driver {
    animator: Animator,
    sequence: &'static str,
    time: f32,
    blend_time: f32,
    blend_in: f32,
    first: bool,
    wrapped: bool,
    looping: bool,
    ended: bool,
    component_to_world: Trs,
}

impl Driver {
    fn new(animator: Animator) -> Driver {
        Driver { animator, sequence: "0001", time: 0.0, blend_time: 0.0, blend_in: 0.2, first: true, wrapped: false, looping: true, ended: false, component_to_world: c2w(Vec3::ZERO, 0.0) }
    }

    fn play(&mut self, sequence: &'static str, blend_in: f32, looping: bool) {
        (self.sequence, self.time, self.blend_time, self.blend_in, self.first, self.wrapped, self.looping, self.ended) = (sequence, 0.0, 0.0, blend_in, true, false, looping, false);
    }

    fn step(&mut self, world: &dyn IkWorld) -> Pose {
        let length = self.animator.clip_info(self.sequence).expect("clip").length;
        let sequences = [self.sequence];
        let mut body = BodySlot { samples: [(self.sequence, 1.0), (self.sequence, 0.0)], blend_with: None, notify_sequences: &sequences, time: self.time, weight: 0.0, active: false, notify_window: (0.0, 0.0) };
        if !self.ended {
            self.blend_time += DT;
            let offset = if self.wrapped { 0.0 } else { 1.0e-4 };
            let from = if std::mem::take(&mut self.first) { -1.0 } else { self.time + offset };
            let mut to = self.time + DT;
            if self.looping && to >= length {
                to %= length;
                self.wrapped = true;
                body.notify_window = (from, to);
            } else {
                if to >= length {
                    to = length;
                    self.ended = true;
                }
                body.notify_window = (from, (to + offset).min(length + offset));
            }
            self.time = to;
            body.time = to;
            body.weight = if self.blend_in <= 0.0 { 1.0 } else { (self.blend_time / self.blend_in).min(1.0) };
            body.active = true;
        }
        self.animator.step(&Frame { dt: DT, body, component_to_world: self.component_to_world }, world).clone()
    }
}

/// A plane through `point` with unit `normal`, solid below.
struct Plane {
    point: Vec3,
    normal: Vec3,
}

impl IkWorld for Plane {
    fn sweep_sphere(&self, start: Vec3, end: Vec3, radius: f32) -> Option<SweepHit> {
        let (d0, d1) = ((start - self.point).dot(self.normal), (end - self.point).dot(self.normal));
        if d0 < radius {
            return Some(SweepHit { time: 0.0, impact_point: start - self.normal * d0, impact_normal: self.normal, start_penetrating: true });
        }
        if d1 > radius {
            return None;
        }
        let time = (d0 - radius) / (d0 - d1);
        let centre = start + (end - start) * time;
        Some(SweepHit { time, impact_point: centre - self.normal * radius, impact_normal: self.normal, start_penetrating: false })
    }
}

/// Ground at height 0, except `high` cm where x > `edge` (centres only: feet stay clear of the edge).
struct Step {
    edge: f32,
    high: f32,
}

impl IkWorld for Step {
    fn sweep_sphere(&self, start: Vec3, end: Vec3, radius: f32) -> Option<SweepHit> {
        let height = if start.x > self.edge { self.high } else { 0.0 };
        Plane { point: Vec3::new(0.0, 0.0, height), normal: Vec3::Z }.sweep_sphere(start, end, radius)
    }
}

// ------------------------------------------------------------------ A4, A5: interpolation rules

#[test]
fn a4_rotations_are_normalised_linear_blends_with_sign_correction() {
    // Spec 7 / 8, change A4. Between keys: FastLerp + normalise, along the shorter arc.
    let (a, b) = (Quat::IDENTITY, Quat::from_rotation_z(120.0f32.to_radians()));
    let got = key_rotation(a, b, 0.25);
    let expected = Quat::from_xyzw(b.x * 0.25, b.y * 0.25, b.z * 0.25, 0.75 + b.w * 0.25).normalize();
    assert!(angle_deg(got, expected) < 1.0e-3);
    let slerp = a.slerp(b, 0.25);
    let difference = angle_deg(got, slerp);
    println!("A4: nlerp against slerp at t = 0.25 over 120 degrees: {difference:.3} degrees apart");
    assert!(difference > 1.0, "it must not be a slerp: {difference}");
    // The shorter arc: the same rotation whatever the sign of a key.
    assert!(angle_deg(key_rotation(a, -b, 0.25), got) < 1.0e-3);
    // Pose blends: the sign that keeps the sum on the side of the running total.
    let t = |q: Quat| Trs { translation: Vec3::ZERO, rotation: q, scale: Vec3::ONE };
    let blended = weighted(&[(t(b), 0.5), (t(-b), 0.5)]);
    assert!(angle_deg(blended.rotation, b) < 1.0e-3, "q and -q are the same rotation: {:?}", blended.rotation);
    let half = slot(&t(b), 0.5, &t(a));
    assert!(angle_deg(half.rotation, a.slerp(b, 0.5)) < 1.0e-3, "half way is half way either way");
}

#[test]
fn a5_scale_is_geometric_between_keys_and_logarithmic_in_blends() {
    // Spec 7 / 8, change A5.
    assert!((key_scale(Vec3::new(1.0, 0.01, 2.0), Vec3::new(4.0, 1.0, 2.0), 0.5) - Vec3::new(2.0, 0.1, 2.0)).length() < 1.0e-5);
    // No common sign: linear.
    assert_eq!(key_scale(Vec3::splat(-1.0), Vec3::splat(1.0), 0.25).x, -0.5);
    let t = |s: f32| Trs { translation: Vec3::ZERO, rotation: Quat::IDENTITY, scale: Vec3::splat(s) };
    let blended = slot(&t(4.0), 0.5, &t(1.0));
    println!("A5: scale 4 over scale 1 at weight 0.5: {} (linear would be 2.5)", blended.scale.x);
    assert!((blended.scale.x - 2.0).abs() < 1.0e-5);
    // Slot edges: weight <= 1e-5 is the source exactly; weight 1 is the montage pose.
    assert_eq!(slot(&t(4.0), 0.0, &t(1.0)), t(1.0));
    assert_eq!(slot(&t(4.0), 1.0, &t(1.0)).scale, Vec3::splat(4.0));
}

// ------------------------------------------------------------------ A1, A8, A12, A14, A16

#[test]
fn a1_the_saved_pose_is_recursive() {
    // Spec 6, change A1 / A2: S[n] = blend(montage pose, S[n-1], w_n), w_n = n dt / blendIn: the
    // old pose's share after k updates is the PRODUCT of (1 - w_i), not 1 - w_k.
    let Some(pack) = pack() else { return };
    let mut animator = Animator::new(pack, Settings { leg_ik: false, kine_driver: false, face: false, ..Settings::default() }).unwrap();
    // S[0] is IdleN at t = 0.
    let idle0 = animator.sample("0001", 0.0);
    assert_eq!(animator.saved_pose(), &idle0[..]);
    let bone = animator.skeleton().index_of("center").unwrap();
    // A held walk pose blended in over 0.2 s.
    let walk = animator.sample("0002", 0.3);
    let sequences = ["0002"];
    let mut share = 1.0;
    for n in 1..=6 {
        let weight = n as f32 * DT / 0.2;
        share *= 1.0 - weight;
        let body = BodySlot { samples: [("0002", 1.0), ("0002", 0.0)], blend_with: None, notify_sequences: &sequences, time: 0.3, weight, active: true, notify_window: (0.0, 0.0) };
        animator.step(&Frame { dt: DT, body, component_to_world: c2w(Vec3::ZERO, 0.0) }, &NoIkWorld);
    }
    let (from, to, now) = (idle0[bone].translation, walk[bone].translation, animator.saved_pose()[bone].translation);
    let measured = (now - to).length() / (from - to).length();
    println!("A1: share of the old pose after 6 updates of a 0.2 s blend-in at 60 Hz: {measured:.4} (product rule {share:.4}; a snapshot blend would give 0.5)");
    assert!((measured - share).abs() < 2.0e-3 && (share - 0.1114).abs() < 1.0e-3);
    // The rotation went the same way (on a bone that turns between the two poses).
    let bone = animator.skeleton().index_of("L_ude1").unwrap();
    let turned = angle_deg(animator.saved_pose()[bone].rotation, walk[bone].rotation) / angle_deg(idle0[bone].rotation, walk[bone].rotation);
    assert!((turned - share).abs() < 0.02, "rotation share {turned}");
}

#[test]
fn a8_after_a_one_shot_ends_the_slot_holds_the_saved_pose() {
    // Spec 8, change A8: an inactive slot returns its source: the last blended pose, bit-exactly,
    // even when the blend-in had not reached weight 1.
    let Some(pack) = pack() else { return };
    let mut animator = Animator::new(pack, Settings { leg_ik: false, kine_driver: false, face: false, ..Settings::default() }).unwrap();
    let sequences = ["0031"];
    let frame = |weight: f32, active: bool| Frame { dt: DT, body: BodySlot { samples: [("0031", 1.0), ("0031", 0.0)], blend_with: None, notify_sequences: &sequences, time: 0.1, weight, active, notify_window: (0.0, 0.0) }, component_to_world: c2w(Vec3::ZERO, 0.0) };
    animator.step(&frame(0.4, true), &NoIkWorld);
    let held = animator.saved_pose().to_vec();
    assert_ne!(held, animator.sample("0031", 0.1), "the blend stopped where it was");
    for _ in 0..5 {
        animator.step(&frame(0.4, false), &NoIkWorld);
        assert_eq!(animator.saved_pose(), &held[..]);
    }
}

#[test]
fn a12_the_root_is_locked_to_the_reference_root_in_root_motion_sequences_only() {
    // Spec 7, change A12; A16 (bones without a track: the mesh reference pose); A11 (never from
    // the last key back to the first).
    let Some(pack) = pack() else { return };
    let animator = Animator::new(pack, Settings::default()).unwrap();
    let reference = animator.skeleton().bones[0].reference;
    assert!(angle_deg(reference.rotation, Quat::from_xyzw(-0.707_106_71, 0.0, 0.0, 0.707_106_83)) < 1.0e-4);
    for sequence in ["0007", "0007L", "0007BR"] {
        for time in [0.0, 0.1, 0.2333] {
            assert_eq!(animator.sample(sequence, time)[0], reference, "{sequence} at {time}");
        }
    }
    // The run loop has no root motion: its root track value is used as it is.
    let root = animator.sample("0003", 0.2)[0];
    let key = Quat::from_xyzw(-0.707_083_34, 0.0, 0.0, 0.707_130_19);
    println!("A12: run loop root rotation {:?} (the clip's key; the locked value is {:?})", root.rotation, reference.rotation);
    assert!((root.rotation - key).length() < 2.0e-6 || (root.rotation + key).length() < 2.0e-6, "{:?}", root.rotation);
    assert_ne!(root.rotation, reference.rotation);
    // A16: a bone without a track.
    let untracked = animator.skeleton().index_of("L_sodeA2").unwrap();
    assert_eq!(animator.sample("0001", 1.0)[untracked], animator.skeleton().bones[untracked].reference);
    // A11: past the end the last key is held.
    assert_eq!(animator.sample("0001", 3.0), animator.sample("0001", 4.0));
}

#[test]
fn a14_drawing_interpolates_two_poses_and_never_re_evaluates() {
    // Change A14: one evaluation per step; the host draws Pose::lerp of the two latest.
    let Some(pack) = pack() else { return };
    let mut driver = Driver::new(Animator::new(pack, Settings::default()).unwrap());
    let a = driver.step(&NoIkWorld);
    let b = driver.step(&NoIkWorld);
    assert_eq!(a.lerp(&b, 0.0).local.len(), a.local.len());
    let bone = driver.animator.skeleton().index_of("mune").unwrap();
    let half = a.lerp(&b, 0.5);
    assert!((half.component[bone].translation - (a.component[bone].translation + b.component[bone].translation) * 0.5).length() < 1.0e-5);
    assert!(angle_deg(a.lerp(&b, 1.0).component[bone].rotation, b.component[bone].rotation) < 1.0e-3);
}

// ------------------------------------------------------------------ B2: face

#[test]
fn b2_the_face_slots_play_from_the_body_clips_notifies() {
    // Spec 12, change B2. IdleN's TresFaceAnim notify at t = 0 (NORMAL_AUTO -> NO_0 -> NO_0_EL
    // = 0000EM, looping, blend-in -1 = the clip's 0.13 s) plays on Face and Mouth on the first
    // update; the montage has weight 0 in that frame and dt / 0.13 on the next.
    let Some(pack) = pack() else { return };
    let mut driver = Driver::new(Animator::new(pack, Settings { leg_ik: false, kine_driver: false, ..Settings::default() }).unwrap());
    let reference: Vec<Trs> = driver.animator.skeleton().reference_pose();
    let first = driver.step(&NoIkWorld);
    let [eyes, mouth] = driver.animator.face_subtrees().map(|s| s.to_vec());
    assert_eq!((eyes.len(), mouth.len()), (30, 26));
    assert_eq!(driver.animator.face_playing().map(|p| p.map(|(clip, _)| clip.to_string())), [Some("0000EM".to_string()), Some("0000EM".to_string())]);
    assert!(eyes.iter().chain(&mouth).all(|&b| first.local[b] == reference[b]), "weight 0 in the frame of the play");
    driver.step(&NoIkWorld);
    let weight = driver.animator.face_playing()[0].unwrap().1;
    assert!((weight - DT / 0.13).abs() < 1.0e-5, "weight after the first update: {weight}");
    let mut pose = first;
    for _ in 0..40 {
        pose = driver.step(&NoIkWorld);
    }
    assert_eq!(driver.animator.face_playing()[0].unwrap().1, 1.0);
    // The subtrees now show 0000EM at the montage's time (it started one update after the body).
    let face_time = 41.0 * DT;
    let clip = driver.animator.sample("0000EM", face_time);
    let worst = eyes.iter().chain(&mouth).map(|&b| angle_deg(pose.local[b].rotation, clip[b].rotation)).fold(0.0, f32::max);
    assert!(worst < 0.05, "the face subtrees follow 0000EM: worst {worst} degrees");
    let moved = eyes.iter().chain(&mouth).map(|&b| angle_deg(pose.local[b].rotation, reference[b].rotation).max((pose.local[b].translation - reference[b].translation).length())).fold(0.0, f32::max);
    println!("B2: the face is not in its reference pose: largest change of a face bone {moved:.3} (degrees or cm)");
    assert!(moved > 0.5, "the face must not be the reference pose: {moved}");
    // Tracks of the face clip outside the two subtrees are discarded: the head is the body clip's.
    let head = driver.animator.skeleton().index_of("atama").unwrap();
    let body = driver.animator.sample("0001", driver.time);
    assert!(angle_deg(pose.local[head].rotation, body[head].rotation) < 1.0e-3);
    // SameCheck: a replay of IdleN does not restart the row that is already the slot's last.
    driver.play("0001", 0.2, true);
    driver.step(&NoIkWorld);
    assert_eq!(driver.animator.face_playing()[0].unwrap().1, 1.0, "not restarted");
    // The fall loop plays 0021EM on both slots (NORMAL_AUTO_ED01 -> NO_21 -> NO_21_EL, blend-in 0).
    driver.play("0033", 0.1, true);
    driver.step(&NoIkWorld);
    assert_eq!(driver.animator.face_playing().map(|p| p.map(|(clip, _)| clip.to_string())), [Some("0021EM".to_string()), Some("0021EM".to_string())]);
    // The run loop's centre sample breathes through the mouth: 0050M at t = 0, 0051M at 0.089 s.
    driver.play("0003", 0.1, true);
    let mut seen = Vec::new();
    for _ in 0..10 {
        driver.step(&NoIkWorld);
        let now = driver.animator.face_playing()[1].unwrap().0.to_string();
        if seen.last() != Some(&now) {
            seen.push(now);
        }
    }
    assert_eq!(seen, ["0050M", "0051M"]);
}

// ------------------------------------------------------------------ B3: KineDriver

#[test]
fn b3_kine_driver_reproduces_the_reference_pose() {
    // Spec 13, check 1: on the reference pose the driver reproduces the reference transform of
    // 99 of the 102 bones it writes; R_susoB1..3 differ by exactly 1 degree (an asset fact).
    let Some(pack) = pack() else { return };
    let mut driver = KineDriver::from_json(pack.kine_driver.as_ref().expect("data/kinedriver.json")).unwrap();
    assert_eq!((driver.operator_count(), driver.written().len()), (313, 102));
    let local = pack.skeleton.reference_pose();
    let base = component_space(&pack.skeleton, &local);
    let mut pose = base.clone();
    driver.evaluate(&mut pose, &local);
    let (mut outside, mut worst) = (Vec::new(), [0.0f32; 3]);
    for &bone in driver.written() {
        let d = [(base[bone].translation - pose[bone].translation).length(), angle_deg(base[bone].rotation, pose[bone].rotation), (base[bone].scale - pose[bone].scale).abs().max_element()];
        if d[0] > 1.0e-3 || d[1] > 1.0e-2 || d[2] > 1.0e-4 {
            outside.push((pack.skeleton.bones[bone].name.clone(), d[1]));
        } else {
            for k in 0..3 {
                worst[k] = worst[k].max(d[k]);
            }
        }
    }
    println!("B3 check 1: outside {outside:?}; worst of the other 99: {:.2e} cm, {:.2e} degrees, {:.2e} scale", worst[0], worst[1], worst[2]);
    assert_eq!(outside.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>(), ["R_susoB1", "R_susoB2", "R_susoB3"]);
    assert!(outside.iter().all(|(_, degrees)| (degrees - 1.0).abs() < 0.01));
}

#[test]
fn b3_kine_driver_equals_the_baked_tracks_at_key_times() {
    // Spec 13, check 2: the `_spo` tracks in the clips are baked copies of the driver's output:
    // at every key time of every body clip the driver's result equals them within 1e-2 cm /
    // 2e-2 degrees / 1e-3 scale (times 10 for clips whose channels are key-reduced to different
    // key times).
    let Some(pack) = pack() else { return };
    let mut driver = KineDriver::from_json(pack.kine_driver.as_ref().expect("data/kinedriver.json")).unwrap();
    let animator = Animator::new(pack.clone(), Settings::default()).unwrap();
    let body: Vec<String> = pack.manifest.get("items").items().iter().filter(|i| i.get("kind").as_str() == Some("animation")).filter_map(|i| i.get("animation_name").as_str().map(str::to_string))
        // (pack 1.5.0 carries every row of the anim set; the clips beyond stage 1 are read on first use and are not part of this check, as in tools/sora_pack.js)
        .filter(|name| pack.clips.contains_key(name))
        .collect();
    assert!(body.len() >= 29, "{} body clips", body.len());
    let (mut total, mut overall) = (0usize, [0.0f32; 3]);
    for name in &body {
        let clip = &pack.clips[name];
        let mut key_times: &[f32] = &[];
        let mut reduced_lengths = Vec::new();
        for track in &clip.tracks {
            for times in [&track.translation.times, &track.rotation.times, &track.scale.times] {
                if times.len() > key_times.len() {
                    key_times = times;
                }
                if times.len() > 1 {
                    reduced_lengths.push(times.len());
                }
            }
        }
        let reduced = reduced_lengths.iter().any(|n| *n != key_times.len());
        let tracked: Vec<usize> = clip.tracks.iter().filter_map(|t| pack.skeleton.index_of(&t.bone)).filter(|b| driver.written().contains(b)).collect();
        let mut worst = [0.0f32; 3];
        for &time in key_times {
            let local = animator.sample(name, time);
            let base = component_space(&pack.skeleton, &local);
            let mut pose = base.clone();
            driver.evaluate(&mut pose, &local);
            for &bone in &tracked {
                let d = [(base[bone].translation - pose[bone].translation).length(), angle_deg(base[bone].rotation, pose[bone].rotation), (base[bone].scale - pose[bone].scale).abs().max_element()];
                for k in 0..3 {
                    worst[k] = worst[k].max(d[k]);
                }
                total += 1;
            }
        }
        let factor = if reduced { 10.0 } else { 1.0 };
        assert!(worst[0] <= 1.0e-2 * factor && worst[1] <= 2.0e-2 * factor && worst[2] <= 1.0e-3 * factor, "{name}: {worst:?} (tolerance factor {factor})");
        for k in 0..3 {
            overall[k] = overall[k].max(worst[k]);
        }
    }
    println!("B3 check 2: {} clips, {total} comparisons; worst {:.2e} cm, {:.2e} degrees, {:.2e} scale", body.len(), overall[0], overall[1], overall[2]);
}

#[test]
fn b3_kine_driver_runs_after_the_graph_and_is_not_linear_between_keys() {
    // Spec 13: between keys the baked tracks interpolate linearly while the driver does not;
    // the runtime value is the driver's. Measured here on the run loop.
    let Some(pack) = pack() else { return };
    let tracked: Vec<usize> = pack.clips["0003"].tracks.iter().filter_map(|t| pack.skeleton.index_of(&t.bone)).collect();
    let mut off = Driver::new(Animator::new(pack.clone(), Settings { leg_ik: false, face: false, kine_driver: false, ..Settings::default() }).unwrap());
    let mut on = Driver::new(Animator::new(pack, Settings { leg_ik: false, face: false, ..Settings::default() }).unwrap());
    off.play("0003", 0.0, true);
    on.play("0003", 0.0, true);
    let written = on.animator.kine_driver_bones().to_vec();
    assert_eq!(written.len(), 102);
    let (mut worst, mut worst_bone, mut unwritten) = ([0.0f32; 2], 0, 0.0f32);
    for _ in 0..32 {
        let (a, b) = (off.step(&NoIkWorld), on.step(&NoIkWorld));
        for bone in 0..a.component.len() {
            let d = [(a.component[bone].translation - b.component[bone].translation).length(), angle_deg(a.component[bone].rotation, b.component[bone].rotation)];
            if written.contains(&bone) {
                // Written bones without a track are at the reference pose without the driver.
                if !tracked.contains(&bone) {
                    continue;
                }
                if d[1] > worst[1] {
                    worst_bone = bone;
                }
                worst = [worst[0].max(d[0]), worst[1].max(d[1])];
            } else if a.component[bone] != b.component[bone] {
                unwritten = unwritten.max(d[0]).max(d[1]).max(f32::MIN_POSITIVE);
            }
        }
    }
    println!("B3: run loop at 60 Hz, driver against baked tracks: up to {:.3} cm / {:.3} degrees on the tracked helper bones (most on {}); other bones {unwritten:.1e}", worst[0], worst[1], on.animator.skeleton().bones[worst_bone].name);
    assert_eq!(unwritten, 0.0, "the driver writes only its own bones");
    assert!(worst[1] > 0.05, "between keys the driver differs from the linear tracks");
}

// ------------------------------------------------------------------ B1: leg IK

#[test]
fn b1_the_alpha_follows_the_tresik_notify_states() {
    // Spec 10 "Alpha", native_D_legik 3.2 - 3.5. IdleN: target 1 over 0.3 s from its first
    // update; the node uses min(alpha, 0.99) and reads the value the previous pawn tick left.
    let Some(pack) = pack() else { return };
    let floor = Plane { point: Vec3::ZERO, normal: Vec3::Z };
    let mut driver = Driver::new(Animator::new(pack, Settings::default()).unwrap());
    driver.step(&floor);
    assert_eq!(driver.animator.leg_ik_report().alpha, 0.0, "the first frame reads the initial 0");
    let a = driver.animator.leg_alpha();
    assert!((a.value - DT / 0.3).abs() < 1.0e-6 && a.target == 1.0, "{a:?}");
    let mut full_at = None;
    for n in 2..=30 {
        driver.step(&floor);
        if full_at.is_none() && driver.animator.leg_alpha().value == 1.0 {
            full_at = Some(n);
        }
    }
    println!("B1 alpha: 1.0 after {full_at:?} updates of IdleN at 60 Hz (0.3 s = 18); the node uses {}", driver.animator.leg_ik_report().alpha);
    assert!(matches!(full_at, Some(18) | Some(19)));
    assert_eq!(driver.animator.leg_ik_report().alpha, ALPHA_CAP);
    // The run start's centre sample: 1 at once (start time 0) until 0.167 s, then to 0 over 0.1 s.
    // The run loop has no window: IdleN's ends -> 0 over its 0.3 s.
    driver.play("0003", 0.1, true);
    driver.step(&floor);
    let a = driver.animator.leg_alpha();
    assert!(a.target == 0.0 && (a.value - (1.0 - DT / 0.3)).abs() < 1.0e-5, "{a:?}");
    for _ in 0..20 {
        driver.step(&floor);
    }
    assert_eq!(driver.animator.leg_alpha().value, 0.0);
    assert_eq!(driver.animator.leg_ik_report().alpha, 0.0, "at alpha 0 the node is not evaluated");
    // The jump start: target 0 over 0.02 s; landing: 1 over 0.1 s.
    driver.play("0034", 0.0, false);
    for _ in 0..7 {
        driver.step(&floor);
    }
    assert_eq!(driver.animator.leg_alpha().value, 1.0, "Land: 1 after 0.1 s");
    // Idle after the landing: the Land state ends and IdleN's begins in one dispatch (end first,
    // then begin): the target stays 1.
    for _ in 0..17 {
        driver.step(&floor);
    }
    assert!(driver.ended, "Land is 0.4 s = 24 updates");
    driver.play("0001", 0.2, true);
    driver.step(&floor);
    assert_eq!((driver.animator.leg_alpha().value, driver.animator.leg_alpha().target), (1.0, 1.0));
}

#[test]
fn b1_on_flat_ground_both_feet_get_the_floor_offset_whatever_the_pose() {
    // Spec 10 step 2 (native_D_legik 4.2 "flat-ground identity"): delta.z = floor Z - component
    // origin Z for both feet; the term the IK absorbs is the capsule float (origin 0.15 above
    // the floor), not the sole offset.
    let Some(pack) = pack() else { return };
    let floor = Plane { point: Vec3::ZERO, normal: Vec3::Z };
    let mut with = Driver::new(Animator::new(pack.clone(), Settings { kine_driver: false, ..Settings::default() }).unwrap());
    let mut without = Driver::new(Animator::new(pack, Settings { kine_driver: false, leg_ik: false, ..Settings::default() }).unwrap());
    with.component_to_world = c2w(Vec3::new(350.0, -120.0, 0.15), 40.0);
    without.component_to_world = with.component_to_world;
    let (mut a, mut b) = (with.step(&floor), without.step(&floor));
    let mut sweeps = Vec::new();
    for _ in 0..150 {
        (a, b) = (with.step(&floor), without.step(&floor));
        sweeps.push(with.animator.leg_ik_report().swept[0]);
    }
    let report = with.animator.leg_ik_report();
    assert!(report.hit[0] && report.hit[1]);
    for delta in report.delta {
        // (On the frames that reuse the cached hit the old sweep fraction meets the current ankle
        // height: the delta is off by the ankle's vertical motion of one frame, as in the game.)
        assert!((delta.z + 0.15).abs() < 0.01 && delta.x.abs() < 1.0e-4 && delta.y.abs() < 1.0e-4, "delta {delta:?}");
    }
    // The hits are cached: a sweep every other frame while he stands (RaycastForceFrameRate 1).
    assert!(sweeps[100..].windows(2).all(|w| w[0] != w[1]), "sweeps {:?}", &sweeps[100..110]);
    // Hips: the controller's steady state is 0.916 x target at 60 Hz for alpha 0.99.
    let ratio = report.hips_drop / report.hips_target;
    println!("B1 flat: delta.z {:.4} / {:.4} cm; hips drop {:.4} of target {:.4} cm = {ratio:.3} (0.916)", report.delta[0].z, report.delta[1].z, report.hips_drop, report.hips_target);
    assert!((report.hips_target - 0.15).abs() < 0.01 && (ratio - 0.916).abs() < 0.012);
    // The ankles come down by the offset (blended in at 0.99): the animation's z = 0 plane is
    // glued to the floor.
    let skeleton = with.animator.skeleton();
    for name in ["L_ashi1", "R_ashi1"] {
        let bone = skeleton.index_of(name).unwrap();
        let moved = a.component[bone].translation.z - b.component[bone].translation.z;
        println!("B1 flat: {name} moved {moved:.4} cm (floor 0.15 below the origin)");
        // ... as far as the hips controller's steady state lets a leg that is already as straight
        // as the animation has it: by the hips drop (0.916 of the offset at 60 Hz).
        assert!(moved < -0.12 && moved > -0.16, "{name} moved {moved}");
    }
}

#[test]
fn b1_on_a_slope_the_feet_follow_the_ground_and_tilt_within_the_clamps() {
    // Spec 10 steps 2 - 5: per ankle delta.z = floor height under it - origin Z + r (1 / cos - 1);
    // the hips drop by the largest downward move; the foot tilts toward the ground normal as a
    // rotator clamped to roll 15 / pitch 10 / yaw 25 degrees.
    let Some(pack) = pack() else { return };
    let slope = 20.0f32.to_radians();
    // Rising toward world +Y (he faces +X: the slope is across him), through the origin.
    let normal = Vec3::new(0.0, -slope.sin(), slope.cos());
    let ground = Plane { point: Vec3::ZERO, normal };
    let mut with = Driver::new(Animator::new(pack.clone(), Settings { kine_driver: false, ..Settings::default() }).unwrap());
    let mut without = Driver::new(Animator::new(pack, Settings { kine_driver: false, leg_ik: false, ..Settings::default() }).unwrap());
    let (mut a, mut b) = (with.step(&ground), without.step(&ground));
    for _ in 0..240 {
        (a, b) = (with.step(&ground), without.step(&ground));
    }
    let report = with.animator.leg_ik_report();
    let skeleton = with.animator.skeleton();
    let feet = ["L_ashi1", "R_ashi1"].map(|n| skeleton.index_of(n).unwrap());
    let world = |t: &Trs| with.component_to_world.rotation * t.translation + with.component_to_world.translation;
    let mut lowest = 0.0f32;
    for (i, &foot) in feet.iter().enumerate() {
        let animated = world(&b.component[foot]);
        let expected = animated.y * slope.tan() + 10.0 * (1.0 / slope.cos() - 1.0);
        println!("B1 slope 20 degrees: foot {i} at y {:.1}: delta.z {:.3} (expected {expected:.3})", animated.y, report.delta[i].z);
        assert!((report.delta[i].z - expected).abs() < 0.02, "foot {i}: delta {:?}, expected z {expected}", report.delta[i]);
        lowest = lowest.min(report.delta[i].z);
        // The foot ends on its target: animated position + delta (hips drop and knee bend).
        let solved = world(&a.component[foot]);
        // The downhill foot stays short of it by what the hips controller leaves (its steady state
        // is 0.916 of the drop) when the leg is already as straight as the animation has it.
        let short = solved.z - (animated.z + report.delta[i].z);
        println!("B1 slope 20 degrees: foot {i} ends {short:.3} cm above its target");
        assert!(short > -0.35 && short < 0.084 * report.hips_target + 0.35, "foot {i} at z {}, target {}", solved.z, animated.z + report.delta[i].z);
        // Tilt: the ground normal seen from the component is a roll or pitch beyond the clamp.
        let tilt = angle_deg(a.component[foot].rotation, b.component[foot].rotation);
        println!("B1 slope 20 degrees: foot {i} tilted {tilt:.2} degrees (clamps: roll 15, pitch 10, yaw 25)");
        assert!(tilt > 5.0 && tilt <= 15.2, "foot {i} tilt {tilt}");
    }
    assert!((report.hips_target + lowest).abs() < 1.0e-3, "hips target {} for the lowest foot {lowest}", report.hips_target);
    assert!((report.hips_drop / report.hips_target - 0.916).abs() < 0.004);
    // Bone lengths are kept by the solve.
    for (thigh, shin, foot) in [("L_momo", "L_sune", "L_ashi1"), ("R_momo", "R_sune", "R_ashi1")] {
        let [t, s, f] = [thigh, shin, foot].map(|n| skeleton.index_of(n).unwrap());
        for (from, to) in [(t, s), (s, f)] {
            let (before, after) = ((b.component[from].translation - b.component[to].translation).length(), (a.component[from].translation - a.component[to].translation).length());
            assert!((before - after).abs() < 0.02, "{thigh}: bone length {before} -> {after}");
        }
    }
}

#[test]
fn b1_a_foot_on_a_step_bends_its_knee_and_the_hips_do_not_rise() {
    // Spec 10 step 3: a foot that must go up never raises the hips.
    let Some(pack) = pack() else { return };
    let mut with = Driver::new(Animator::new(pack.clone(), Settings { kine_driver: false, ..Settings::default() }).unwrap());
    let mut without = Driver::new(Animator::new(pack, Settings { kine_driver: false, leg_ik: false, ..Settings::default() }).unwrap());
    // He faces +Y, so his left foot is toward -X ... find which foot is at larger world x.
    with.component_to_world = c2w(Vec3::new(0.0, 0.0, 0.15), 90.0);
    without.component_to_world = with.component_to_world;
    let world = Step { edge: 0.0, high: 12.0 };
    let (mut a, mut b) = (with.step(&world), without.step(&world));
    for _ in 0..240 {
        (a, b) = (with.step(&world), without.step(&world));
    }
    let report = with.animator.leg_ik_report();
    let skeleton = with.animator.skeleton();
    let feet = ["L_ashi1", "R_ashi1"].map(|n| skeleton.index_of(n).unwrap());
    let x = feet.map(|f| (with.component_to_world.rotation * b.component[f].translation).x);
    let (up, down) = if x[0] > x[1] { (0, 1) } else { (1, 0) };
    println!("B1 step 12 cm: deltas {:.3} / {:.3}; hips target {:.3}, drop {:.3}", report.delta[up].z, report.delta[down].z, report.hips_target, report.hips_drop);
    assert!((report.delta[up].z - 11.85).abs() < 0.01 && (report.delta[down].z + 0.15).abs() < 0.01);
    assert!((report.hips_target - 0.15).abs() < 0.01, "only the downward foot drives the hips");
    let hips = skeleton.index_of("center").unwrap();
    assert!(a.component[hips].translation.z < b.component[hips].translation.z);
    let raised = a.component[feet[up]].translation.z - b.component[feet[up]].translation.z;
    assert!((raised - 11.85).abs() < 0.3, "the foot on the step went up {raised} cm");
    let knee = |pose: &Pose, i: usize| {
        let (foot, shin) = (feet[i], skeleton.bones[feet[i]].parent.unwrap());
        let thigh = skeleton.bones[shin].parent.unwrap();
        let (p, q, r) = (pose.component[thigh].translation, pose.component[shin].translation, pose.component[foot].translation);
        (p - q).angle_between(r - q).to_degrees()
    };
    println!("B1 step 12 cm: knee angle of the raised leg {:.1} -> {:.1} degrees", knee(&b, up), knee(&a, up));
    assert!(knee(&a, up) < knee(&b, up) - 10.0);
}

#[test]
fn b1_the_maya_solve_keeps_lengths_reaches_the_target_and_bends_toward_the_pole() {
    // Spec 10 step 4 (native_D_legik 4.5).
    let (root, joint, foot) = (Vec3::new(0.0, 0.0, 80.0), Vec3::new(0.0, 4.0, 44.0), Vec3::new(0.0, 0.0, 4.0));
    let target = Vec3::new(5.0, 10.0, 20.0);
    let pole = Vec3::new(0.0, 50.0, -30.0);
    let (start, mid) = maya_solve(root, joint, foot, target, pole, 0.0);
    let new_joint = root + start * (joint - root);
    let new_foot = root + start * ((joint - root) + mid * (foot - joint));
    assert!((new_foot - target).length() < 1.0e-3, "foot at {new_foot:?}");
    assert!(((new_joint - root).length() - (joint - root).length()).abs() < 1.0e-3);
    assert!(((new_foot - new_joint).length() - (foot - joint).length()).abs() < 1.0e-3);
    // The knee lies in the plane of root -> target and the pole, on the pole's side.
    let axis = (target - root).normalize();
    let off = |v: Vec3| v - axis * v.dot(axis);
    let (knee, wanted) = (off(new_joint - root), off(pole));
    assert!(knee.normalize().dot(wanted.normalize()) > 0.999, "knee direction {knee:?}, pole {wanted:?}");
    // Out of reach: the leg is straight along root -> target.
    let far = Vec3::new(0.0, 0.0, -200.0);
    let (start, mid) = maya_solve(root, joint, foot, far, pole, 0.0);
    let reach = root + start * ((joint - root) + mid * (foot - joint));
    let length = (joint - root).length() + (foot - joint).length();
    assert!(((reach - root).length() - length).abs() < 1.0e-2 && (reach - root).normalize().dot((far - root).normalize()) > 0.9999);
}
