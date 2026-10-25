//! (ER70) HANDS AND FEET OF A BORROWED ACTION, FOR WHATEVER SORA WEARS.
//!
//! When his figure is the body's action retargeted (figure.rs, sora.rs `own_frame`: a door, a lever, an item; ANIMS's
//! propfit.rs places them at his proportions) each hand (and a planted foot) is brought to its goal by a two-bone solve,
//! AS FAR AS THAT LIMB REACHES.
//!
//! THE WORN OUTFIT (kh-er-render outfit.rs, the OUTFITS agent's): the renderer poses the outfit's own skeleton from
//! Sora's FINAL pose (his local rotations as changes from his reference; his translations scaled by the outfit's hip
//! height), so the pose reaches every outfit. But an outfit's arms and legs have their own lengths: the solve is made ON
//! THE OUTFIT'S SKELETON (its pose from Sora's, `Retarget::pose_full`) and its result written back into Sora's local
//! rotations by the outfit's rule run backwards (`Retarget::host_rotation_for`), so that the outfit drawn holds the goal.
//! For KH3's own look (no outfit) the solve is kh-anim's `reach_arm` on Sora's skeleton. (LADDER2: the ladders' rung rule
//! is gone with the climb.)

use glam::{Quat, Vec3};
use kh_anim::Trs;
use kh_er_render::outfit::Retarget;

/// The rows of a limb: upper bone, lower bone, end (Sora's / every outfit's bone names).
pub const LIMBS: [[&str; 3]; 4] = [["L_ude1", "L_ude2", "L_te"], ["R_ude1", "R_ude2", "R_te"], ["L_momo", "L_sune", "L_ashi1"], ["R_momo", "R_sune", "R_ashi1"]];

/// THE TWO-BONE SOLVE (component space): `a`, `b`, `c` the upper bone's, the lower bone's and the end's positions, `t`
/// the target. The end goes to `t` or, beyond the limb's length, as far toward it as the limb reaches; the limb bends on
/// the side it is bent now. Returns the rotation of the upper bone (about `a`) and then of the lower bone (about the new
/// `b`), both as changes in component space; None for a limb of no length.
pub fn two_bone(a: Vec3, b: Vec3, c: Vec3, t: Vec3) -> Option<(Quat, Quat)> {
    let (l1, l2) = ((b - a).length(), (c - b).length());
    let to = t - a;
    if l1 < 1.0e-3 || l2 < 1.0e-3 || to.length() < 1.0e-3 || !to.is_finite() {
        return None;
    }
    let d = to.length().clamp((l1 - l2).abs() + 1.0e-3, l1 + l2 - 1.0e-3);
    let u = to.normalize();
    // the bend: where the joint is now, off the line to the target (else any side across the limb)
    let ab = b - a;
    let mut v = ab - u * ab.dot(u);
    if v.length() < 1.0e-4 {
        v = (c - b) - u * (c - b).dot(u);
    }
    if v.length() < 1.0e-4 {
        v = u.any_orthonormal_vector();
    }
    let v = v.normalize();
    let cos_a = ((l1 * l1 + d * d - l2 * l2) / (2.0 * l1 * d)).clamp(-1.0, 1.0);
    let b_new = a + (u * cos_a + v * (1.0 - cos_a * cos_a).max(0.0).sqrt()) * l1;
    let q1 = Quat::from_rotation_arc(ab / l1, (b_new - a) / l1);
    let c_turned = a + q1 * (c - a);
    let end = a + u * d;
    let q2 = Quat::from_rotation_arc((c_turned - b_new).normalize(), (end - b_new).normalize());
    Some((q1, q2))
}

/// One limb of SORA'S OWN skeleton (KH3's look) to `target` (component space, cm), as far as it reaches.
#[cfg_attr(not(feature = "party"), allow(dead_code))]
pub fn reach_sora(skeleton: &kh_anim::Skeleton, local: &mut [Trs], limb: usize, target: Vec3, weight: f32) {
    let names = LIMBS[limb];
    let (Some(u), Some(l), Some(e)) = (skeleton.index_of(names[0]), skeleton.index_of(names[1]), skeleton.index_of(names[2])) else { return };
    kh_anim::retarget::reach_arm(skeleton, u, l, e, target, weight, local);
}

/// One limb of THE WORN OUTFIT to `target` (component space, cm): solved on the outfit's skeleton, written back into
/// Sora's local rotations (upper, lower; the end keeps its turn in space). False when the outfit lacks the bones.
pub fn reach_outfit(r: &Retarget, local: &mut [Trs], limb: usize, target: Vec3, weight: f32) -> bool {
    let names = LIMBS[limb];
    let (Some(u), Some(l), Some(e)) = (r.index_of(names[0]), r.index_of(names[1]), r.index_of(names[2])) else { return false };
    let (_, oc) = r.pose_full(local, &[]);
    let (Some(cu), Some(cl), Some(ce)) = (oc.get(u).copied(), oc.get(l).copied(), oc.get(e).copied()) else { return false };
    let (a, b, c) = (cu.translation, cl.translation, ce.translation);
    let Some((q1, q2)) = two_bone(a, b, c, target) else { return false };
    let rot = |k: Option<usize>| k.and_then(|k| oc.get(k)).map_or(Quat::IDENTITY, |t| t.rotation);
    // the new component rotations: the upper bone and everything under it turned by q1, the lower one and under it by q2 too
    let up_new = q1 * cu.rotation;
    let low_parent_new = if r.parent_of(l) == Some(u) { up_new } else { q1 * rot(r.parent_of(l)) };
    let low_new = q2 * q1 * cl.rotation;
    let end_parent_new = if r.parent_of(e) == Some(l) { low_new } else { q2 * q1 * rot(r.parent_of(e)) };
    let writes = [
        (u, rot(r.parent_of(u)).inverse() * up_new),
        (l, low_parent_new.inverse() * low_new),
        // (the end keeps its rotation in space: a hand on a handle, a foot flat on the floor)
        (e, end_parent_new.inverse() * ce.rotation),
    ];
    for (bone, outfit_local) in writes {
        if let Some((h, q)) = r.host_rotation_for(bone, outfit_local) {
            if let Some(t) = local.get_mut(h) {
                t.rotation = t.rotation.slerp(q, weight.clamp(0.0, 1.0)).normalize();
            }
        }
    }
    true
}

/// The worn outfit's retarget, read from the pack once per outfit (the renderer's own id: what is drawn).
#[derive(Default)]
pub struct Worn {
    id: String,
    rig: Option<std::sync::Arc<Retarget>>,
}

static PACK: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();

/// Where the pack is (set once when Sora is made).
pub fn set_pack(dir: &std::path::Path) {
    let _ = PACK.set(dir.to_path_buf());
}

impl Worn {
    /// The outfit drawn now (None: KH3's own look, or its skeleton could not be read: said once).
    pub fn current(&mut self, host: &kh_anim::Skeleton) -> Option<std::sync::Arc<Retarget>> {
        let id = kh_er_render::outfit::loaded();
        if id != self.id {
            self.id = id.clone();
            self.rig = match PACK.get().map(|pack| kh_er_render::outfit::retarget_for(pack, &id, host)) {
                Some(Ok(rig)) => rig.map(std::sync::Arc::new),
                Some(Err(e)) => {
                    crate::erlog!("own: THE WORN OUTFIT'S SKELETON IS NOT READABLE ({id}: {e}): hands and feet by Sora's own skeleton");
                    None
                }
                None => None,
            };
            if self.rig.is_some() {
                crate::erlog!("own: hands and feet of the borrowed actions for the worn outfit {id} (its own skeleton)");
            }
        }
        self.rig.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The solve: the end reaches a target within the limb's length, keeps both lengths, bends on its own side; a target
    /// out of reach gets the straight limb pointing at it.
    #[test]
    fn the_two_bone_solve() {
        let (a, b, c) = (Vec3::ZERO, Vec3::new(0.0, 5.0, -30.0), Vec3::new(0.0, 0.0, -60.0));
        let t = Vec3::new(20.0, 10.0, -40.0);
        let (q1, q2) = two_bone(a, b, c, t).unwrap();
        let b2 = a + q1 * (b - a);
        let c2 = b2 + q2 * (q1 * (c - b));
        assert!((c2 - t).length() < 0.05, "{c2}");
        assert!(((b2 - a).length() - (b - a).length()).abs() < 1e-3 && ((c2 - b2).length() - (c - b).length()).abs() < 1e-3);
        assert!((b2 - a).dot(Vec3::Y) > 0.0 || (b2 - (a + t) * 0.5).y > -1.0, "bent on the side it was bent: {b2}");
        let far = Vec3::new(0.0, 0.0, -200.0);
        let (q1, q2) = two_bone(a, b, c, far).unwrap();
        let b2 = a + q1 * (b - a);
        let c2 = b2 + q2 * (q1 * (c - b));
        assert!(((c2 - a).length() - ((b - a).length() + (c - b).length())).abs() < 0.1, "straight: {c2}");
        assert!((c2.normalize() - Vec3::NEG_Z).length() < 0.02);
        assert!(two_bone(a, a, c, t).is_none());
    }

    /// EVERY OUTFIT OF THE PACK (`build/sora_pack/outfits/*`): a pose of Sora (his own `GripLoop` frame: arms up), each hand
    /// sent to a goal 15 cm in front of and above his shoulder, each foot to one in front of his hip: after the solve
    /// written back into Sora's pose, THE OUTFIT'S OWN POSE (the renderer's `pose_full`) has its hands and feet on those
    /// goals. Skipped (passes, says so) without the pack.
    #[test]
    fn every_outfit_reaches_its_goals() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_pack");
        let Ok(pack) = kh_anim::Pack::load(&root) else {
            eprintln!("no pack at {}: skipped", root.display());
            return;
        };
        let Ok(animator) = kh_anim::Animator::new(pack, kh_anim::Settings::default()) else { return };
        let skeleton = animator.skeleton().clone();
        let base = {
            let s = animator.sample(kh_core::AnimId::GripLoop.sequences()[0], 0.5);
            if s.len() == skeleton.len() { s } else { skeleton.reference_pose() }
        };
        let mut ids: Vec<String> = std::fs::read_dir(root.join("outfits")).map(|d| d.filter_map(|e| e.ok()).filter(|e| e.path().join("manifest.json").is_file()).map(|e| e.file_name().to_string_lossy().to_string()).collect()).unwrap_or_default();
        ids.sort();
        eprintln!("outfits: {ids:?}");
        assert!(!ids.is_empty() || !root.join("outfits").is_dir());
        for id in ids {
            let r = kh_er_render::outfit::retarget_for(&root, &id, &skeleton).unwrap().unwrap();
            let mut local = base.clone();
            let (_, oc) = r.pose_full(&local, &[]);
            let pos = |n: &str| r.index_of(n).map(|k| oc[k].translation).unwrap();
            // goals: hands 25 cm in front of the body (Sora faces the component's +Y), 10 cm above the shoulders; feet 20 cm in
            // front of the hips, 45 cm below them (within every outfit's reach)
            let fwd = Vec3::Y;
            let goals = [pos("L_ude1") + fwd * 25.0 + Vec3::Z * 10.0, pos("R_ude1") + fwd * 25.0 + Vec3::Z * 10.0, pos("L_momo") + fwd * 20.0 - Vec3::Z * 45.0, pos("R_momo") + fwd * 20.0 - Vec3::Z * 45.0];
            for (limb, goal) in goals.iter().enumerate() {
                assert!(reach_outfit(&r, &mut local, limb, *goal, 1.0), "{id}: limb {limb}");
            }
            let (_, after) = r.pose_full(&local, &[]);
            for (limb, goal) in goals.iter().enumerate() {
                let end = after[r.index_of(LIMBS[limb][2]).unwrap()].translation;
                let missed = (end - *goal).length();
                eprintln!("{id}: {} -> {:.1} cm from its goal", LIMBS[limb][2], missed);
                assert!(missed < 2.0, "{id}: the {} is {missed:.1} cm off its goal (at {end}, goal {goal})", LIMBS[limb][2]);
            }
        }
    }
}
