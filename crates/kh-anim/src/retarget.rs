//! Retargeting a pose of ANOTHER RIG onto this skeleton (IMPL_ER16: the figure another game animates in a
//! cutscene, shown as Sora). Engine-free; nothing of KH3's is in here.
//!
//! The source rig is given by its bind pose (model space) and, each frame, by its bones' WORLD transforms; both in
//! the TARGET'S axes and units (the caller converts). Per mapped bone the world rotation of the target bone is
//! `source_now * source_bind^-1 * fix * align * target_reference`, where
//! - `align` turns the target's reference pose so that its body frame (up = pelvis to head, across = right thigh to
//!   left thigh) is the source's bind body frame, and
//! - `fix` turns the target bone's reference direction (bone to its aim child) into the source bone's bind
//!   direction, so that the two rigs' different reference poses (T against A, bent knees) do not matter: in every
//!   pose the target bone points where the source bone points.
//! The root bone's place is the source pelvis scaled about the floor point under it by the leg-length ratio, so the
//! shorter figure's feet stay on the floor.

use glam::{Mat3, Quat, Vec3};

use crate::pack::Skeleton;
use crate::pose::component_space;
use crate::Trs;

/// One mapped bone: the source bone's name, the target's, and (optional) the child of each that the bone points at.
#[derive(Clone, Copy, Debug)]
pub struct MapRow {
    pub source: &'static str,
    pub target: &'static str,
    pub aim: Option<(&'static str, &'static str)>,
}

/// The source rig's bones the body frame is taken from, and the target's.
#[derive(Clone, Copy, Debug)]
pub struct BodyFrame {
    pub pelvis: (&'static str, &'static str),
    pub head: (&'static str, &'static str),
    pub left_thigh: (&'static str, &'static str),
    pub right_thigh: (&'static str, &'static str),
    /// A foot (the leg length is pelvis height above it in the bind / reference pose).
    pub foot: (&'static str, &'static str),
}

#[derive(Clone, Debug)]
struct Pair {
    /// Index into the rows (the caller's per-frame source list is in row order).
    target: usize,
    bind_inverse: Quat,
    /// `fix * align * target_reference` (component space of the target's reference pose).
    offset: Quat,
}

#[derive(Clone, Debug)]
pub struct Retarget {
    pairs: Vec<Pair>,
    by_target: Vec<Option<usize>>,
    /// Target leg length / source leg length.
    pub ratio: f32,
    /// The row whose source bone carries the body's place (the pelvis): its target bone is the one that is moved.
    root_row: usize,
}

fn body_frame(pelvis: Vec3, head: Vec3, left: Vec3, right: Vec3) -> Option<Mat3> {
    let up = (head - pelvis).try_normalize()?;
    let across = left - right;
    let across = (across - up * across.dot(up)).try_normalize()?;
    Some(Mat3::from_cols(across, up, across.cross(up)))
}

impl Retarget {
    /// `source_bind(name)`: the source bone's bind transform in its model space (target axes and units).
    /// The target bone the source pelvis is mapped to is the one that is placed (map it to the body's root bone).
    pub fn new(target: &Skeleton, rows: &[MapRow], frame: &BodyFrame, source_bind: &dyn Fn(&str) -> Option<(Vec3, Quat)>) -> Result<Retarget, String> {
        let reference = component_space(target, &target.reference_pose());
        let t = |name: &str| target.index_of(name).ok_or_else(|| format!("retarget: the target has no bone {name}"));
        let s = |name: &str| source_bind(name).ok_or_else(|| format!("retarget: the source has no bone {name}"));
        let source_frame = body_frame(s(frame.pelvis.0)?.0, s(frame.head.0)?.0, s(frame.left_thigh.0)?.0, s(frame.right_thigh.0)?.0).ok_or("retarget: the source's body frame is degenerate")?;
        let target_frame = body_frame(reference[t(frame.pelvis.1)?].translation, reference[t(frame.head.1)?].translation, reference[t(frame.left_thigh.1)?].translation, reference[t(frame.right_thigh.1)?].translation).ok_or("retarget: the target's body frame is degenerate")?;
        let align = Quat::from_mat3(&(source_frame * target_frame.transpose())).normalize();
        let up_s = source_frame.y_axis;
        let up_t = target_frame.y_axis;
        let source_leg = (s(frame.pelvis.0)?.0 - s(frame.foot.0)?.0).dot(up_s).abs();
        let target_leg = (reference[t(frame.pelvis.1)?].translation - reference[t(frame.foot.1)?].translation).dot(up_t).abs();
        if source_leg < 1.0e-4 || target_leg < 1.0e-4 {
            return Err("retarget: a leg has no length".to_string());
        }
        let mut pairs = Vec::new();
        let mut by_target = vec![None; target.len()];
        for (k, row) in rows.iter().enumerate() {
            let (bone, (source_position, source_rotation)) = (t(row.target)?, s(row.source)?);
            let fix = match row.aim {
                Some((source_aim, target_aim)) => {
                    let to = (s(source_aim)?.0 - source_position).try_normalize();
                    let from = (align * (reference[t(target_aim)?].translation - reference[bone].translation)).try_normalize();
                    match (from, to) {
                        (Some(from), Some(to)) => Quat::from_rotation_arc(from, to),
                        _ => Quat::IDENTITY,
                    }
                }
                None => Quat::IDENTITY,
            };
            if by_target[bone].is_some() {
                return Err(format!("retarget: {} is mapped twice", row.target));
            }
            by_target[bone] = Some(k);
            pairs.push(Pair { target: bone, bind_inverse: source_rotation.conjugate(), offset: (fix * align * reference[bone].rotation).normalize() });
        }
        let root_row = rows.iter().position(|r| r.source == frame.pelvis.0).ok_or("retarget: the pelvis is not mapped")?;
        Ok(Retarget { by_target, ratio: target_leg / source_leg, root_row, pairs })
    }

    /// The number of source transforms `apply` wants (one per row, in row order).
    pub fn rows(&self) -> usize {
        self.pairs.len()
    }

    /// Where the pelvis-mapped target bone goes: the source pelvis, scaled about the floor point under it (`up` = the
    /// world's up axis, `floor` = the floor's height along it).
    pub fn pelvis_place(&self, source_pelvis: Vec3, up: Vec3, floor: f32) -> Vec3 {
        let height = source_pelvis.dot(up) - floor;
        source_pelvis + up * (height * self.ratio - height)
    }

    /// Writes the retargeted pose into `local` (the target's local pose; bones that are not mapped keep what they
    /// have), blended by `weight`. `source`: the world transform (position, rotation) of each row's source bone;
    /// `pelvis_world`: where the pelvis-mapped bone goes (see `pelvis_place`); `component_to_world`: the target's.
    pub fn apply(&self, skeleton: &Skeleton, source: &[(Vec3, Quat)], pelvis_world: Vec3, component_to_world: &Trs, weight: f32, local: &mut [Trs]) {
        if source.len() != self.pairs.len() || local.len() != skeleton.len() || weight <= 0.0 {
            return;
        }
        let weight = weight.min(1.0);
        let to_component = component_to_world.rotation.conjugate();
        let world_rotation = |k: usize| (source[k].1 * self.pairs[k].bind_inverse * self.pairs[k].offset).normalize();
        let root_target = self.pairs[self.root_row].target;
        let mut component: Vec<Trs> = Vec::with_capacity(local.len());
        for (bone, info) in skeleton.bones.iter().enumerate() {
            let mut c = match info.parent {
                Some(parent) => local[bone].then(&component[parent]),
                None => local[bone],
            };
            let mut changed = false;
            if bone == root_target {
                let want = to_component * (pelvis_world - component_to_world.translation);
                c.translation = c.translation.lerp(want, weight);
                changed = true;
            }
            if let Some(k) = self.by_target[bone] {
                let want = (to_component * world_rotation(k)).normalize();
                c.rotation = c.rotation.slerp(want, weight).normalize();
                changed = true;
            }
            if changed {
                local[bone] = match info.parent {
                    Some(parent) => c.relative_to(&component[parent]),
                    None => c,
                };
            }
            component.push(c);
        }
    }
}

impl Retarget {
    /// The target bone of a row.
    pub fn target_bone(&self, row: usize) -> Option<usize> {
        self.pairs.get(row).map(|p| p.target)
    }
}

/// (IMPL_ER17) A HAND BROUGHT TO A PLACE: after `Retarget::apply` the target's arm points as the source's does, but a
/// shorter arm ends short of what the source's hand touches. This turns the upper and the lower bone of one arm (a
/// two-bone solve in the plane the arm is bent in now) so that the hand bone is at `to` (the target's component space),
/// or as near as the arm reaches; the hand keeps its rotation. `weight` blends from the pose as it is.
pub fn reach_arm(skeleton: &Skeleton, upper: usize, lower: usize, hand: usize, to: Vec3, weight: f32, local: &mut [Trs]) {
    if weight <= 0.0 || local.len() != skeleton.len() || upper >= local.len() || lower >= local.len() || hand >= local.len() || !to.is_finite() {
        return;
    }
    let weight = weight.min(1.0);
    let c = component_space(skeleton, local);
    let (s, e, h) = (c[upper].translation, c[lower].translation, c[hand].translation);
    let (l1, l2) = ((e - s).length(), (h - e).length());
    if l1 < 1.0e-4 || l2 < 1.0e-4 {
        return;
    }
    let Some(dir) = (to - s).try_normalize() else { return };
    let dist = (to - s).length().clamp((l1 - l2).abs() + 1.0e-3, (l1 + l2) * 0.999);
    // the plane the arm is bent in now; a straight arm bends where the hand's offset from the line points, else anywhere across
    let across = |v: Vec3| (v - dir * v.dot(dir)).try_normalize();
    let pole = across(e - s).or_else(|| across(h - s)).or_else(|| across(Vec3::Z)).or_else(|| across(Vec3::X)).unwrap_or(Vec3::Y);
    let cos = ((l1 * l1 + dist * dist - l2 * l2) / (2.0 * l1 * dist)).clamp(-1.0, 1.0);
    let elbow = s + dir * (l1 * cos) + pole * (l1 * (1.0 - cos * cos).max(0.0).sqrt());
    let wrist = s + dir * dist;
    let first = Quat::from_rotation_arc((e - s) / l1, (elbow - s).normalize_or_zero());
    let second = Quat::from_rotation_arc((first * (h - e)).normalize_or_zero(), (wrist - elbow).normalize_or_zero());
    let hand_rotation = c[hand].rotation;
    let set = |bone: usize, rotation: Quat, local: &mut [Trs]| {
        let c = component_space(skeleton, local);
        let mut now = c[bone];
        now.rotation = now.rotation.slerp(rotation, weight).normalize();
        local[bone] = match skeleton.bones[bone].parent {
            Some(parent) => now.relative_to(&c[parent]),
            None => now,
        };
    };
    set(upper, (first * c[upper].rotation).normalize(), local);
    set(lower, (second * first * c[lower].rotation).normalize(), local);
    // (the hand keeps the rotation it had in the component)
    let c2 = component_space(skeleton, local);
    let mut now = c2[hand];
    now.rotation = hand_rotation;
    if let Some(parent) = skeleton.bones[hand].parent {
        local[hand] = now.relative_to(&c2[parent]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json::Json;

    /// A small biped whose reference pose is a T: pelvis at height 50, head 100, thighs +-10 across (Y), feet at 0;
    /// faces +X, up +Z. Bones have identity rotations, so component translation differences are plain.
    fn biped() -> Skeleton {
        let bone = |name: &str, parent: i64, t: [f32; 3]| format!(r#"{{"name":"{name}","parent":{parent},"translation_ue":[{},{},{}],"rotation_ue":[0,0,0,1],"scale_ue":[1,1,1]}}"#, t[0], t[1], t[2]);
        let bones = [
            bone("root", -1, [0.0, 0.0, 0.0]),
            bone("pelvis", 0, [0.0, 0.0, 50.0]),
            bone("spine", 1, [0.0, 0.0, 25.0]),
            bone("head", 2, [0.0, 0.0, 25.0]),
            bone("l_thigh", 1, [0.0, -10.0, 0.0]),
            bone("l_foot", 4, [0.0, 0.0, -50.0]),
            bone("r_thigh", 1, [0.0, 10.0, 0.0]),
            bone("r_foot", 6, [0.0, 0.0, -50.0]),
            bone("l_arm", 2, [0.0, -20.0, 20.0]),
            bone("l_hand", 8, [0.0, -30.0, 0.0]),
        ];
        let text = format!(r#"{{"skeleton":{{"bones":[{}]}}}}"#, bones.join(","));
        Skeleton::from_manifest(&Json::parse(&text).unwrap()).unwrap()
    }

    /// The source: twice as tall, the same axes, arms hanging DOWN in its bind pose (an "I" pose), every bone's
    /// bind rotation a quarter turn about X (rigs differ in their bones' own axes).
    fn source_bind(name: &str) -> Option<(Vec3, Quat)> {
        let twist = Quat::from_rotation_x(std::f32::consts::FRAC_PI_2);
        let p = match name {
            "Pelvis" => Vec3::new(0.0, 0.0, 100.0),
            "Spine" => Vec3::new(0.0, 0.0, 150.0),
            "Head" => Vec3::new(0.0, 0.0, 200.0),
            "L_Thigh" => Vec3::new(0.0, -20.0, 100.0),
            "L_Foot" => Vec3::new(0.0, -20.0, 0.0),
            "R_Thigh" => Vec3::new(0.0, 20.0, 100.0),
            "R_Foot" => Vec3::new(0.0, 20.0, 0.0),
            "L_Arm" => Vec3::new(0.0, -40.0, 190.0),
            "L_Hand" => Vec3::new(0.0, -40.0, 130.0),
            _ => return None,
        };
        Some((p, twist))
    }

    const ROWS: [MapRow; 7] = [
        MapRow { source: "Pelvis", target: "pelvis", aim: None },
        MapRow { source: "Spine", target: "spine", aim: Some(("Head", "head")) },
        MapRow { source: "Head", target: "head", aim: None },
        MapRow { source: "L_Thigh", target: "l_thigh", aim: Some(("L_Foot", "l_foot")) },
        MapRow { source: "R_Thigh", target: "r_thigh", aim: Some(("R_Foot", "r_foot")) },
        MapRow { source: "L_Arm", target: "l_arm", aim: Some(("L_Hand", "l_hand")) },
        MapRow { source: "L_Hand", target: "l_hand", aim: None },
    ];
    const FRAME: BodyFrame = BodyFrame { pelvis: ("Pelvis", "pelvis"), head: ("Head", "head"), left_thigh: ("L_Thigh", "l_thigh"), right_thigh: ("R_Thigh", "r_thigh"), foot: ("L_Foot", "l_foot") };

    fn posed(whole: Quat, shift: Vec3) -> Vec<(Vec3, Quat)> {
        ROWS.iter().map(|r| source_bind(r.source).unwrap()).map(|(p, q)| (whole * p + shift, whole * q)).collect()
    }

    fn close(a: Vec3, b: Vec3) -> bool {
        (a - b).length() < 1.0e-3
    }

    #[test]
    fn the_bind_pose_gives_the_reference_with_the_source_s_limb_directions() {
        let skeleton = biped();
        let retarget = Retarget::new(&skeleton, &ROWS, &FRAME, &source_bind).unwrap();
        assert!((retarget.ratio - 0.5).abs() < 1.0e-5);
        let mut local = skeleton.reference_pose();
        let source = posed(Quat::IDENTITY, Vec3::ZERO);
        let place = retarget.pelvis_place(source[0].0, Vec3::Z, 0.0);
        assert!(close(place, Vec3::new(0.0, 0.0, 50.0)));
        retarget.apply(&skeleton, &source, place, &Trs::IDENTITY, 1.0, &mut local);
        let c = component_space(&skeleton, &local);
        let at = |n: &str| c[skeleton.index_of(n).unwrap()].translation;
        // legs and spine as the reference; THE ARM HANGS DOWN as the source's does (the reference holds it out)
        assert!(close(at("pelvis"), Vec3::new(0.0, 0.0, 50.0)));
        assert!(close(at("head"), Vec3::new(0.0, 0.0, 100.0)));
        assert!(close(at("l_foot"), Vec3::new(0.0, -10.0, 0.0)));
        assert!(close(at("l_hand") - at("l_arm"), Vec3::new(0.0, 0.0, -30.0)));
    }

    #[test]
    fn a_source_lying_down_lays_the_target_down_on_the_floor() {
        let skeleton = biped();
        let retarget = Retarget::new(&skeleton, &ROWS, &FRAME, &source_bind).unwrap();
        // the whole source tipped forward onto its face (about Y), its pelvis 12 above a floor at height 300
        let whole = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
        let bind_pelvis = source_bind("Pelvis").unwrap().0;
        let shift = Vec3::new(500.0, 40.0, 312.0) - whole * bind_pelvis;
        let source = posed(whole, shift);
        let place = retarget.pelvis_place(source[0].0, Vec3::Z, 300.0);
        assert!(close(place, Vec3::new(500.0, 40.0, 306.0)));
        // a component that stands somewhere else, turned: the result in the world must not depend on it
        let to_world = Trs { translation: Vec3::new(480.0, 0.0, 300.0), rotation: Quat::from_rotation_z(1.1), scale: Vec3::ONE };
        let mut local = skeleton.reference_pose();
        retarget.apply(&skeleton, &source, place, &to_world, 1.0, &mut local);
        let c = component_space(&skeleton, &local);
        let world = |n: &str| to_world.transform_point(c[skeleton.index_of(n).unwrap()].translation);
        assert!(close(world("pelvis"), place));
        // the body lies along +X (the source's up went to +X), half as long as the source
        assert!(close(world("head") - world("pelvis"), Vec3::new(50.0, 0.0, 0.0)));
        assert!(close(world("l_foot") - world("l_thigh"), Vec3::new(-50.0, 0.0, 0.0)));
        // weight 0 changes nothing; weight 0.5 is in between
        let mut untouched = skeleton.reference_pose();
        retarget.apply(&skeleton, &source, place, &to_world, 0.0, &mut untouched);
        assert_eq!(untouched, skeleton.reference_pose());
    }

    #[test]
    fn an_arm_reaches_the_place_or_as_far_as_it_can() {
        // (the biped's arm has one bone; here: shoulder at (0, 0, 100), elbow 30 along -Y, hand 30 further)
        let bone = |name: &str, parent: i64, t: [f32; 3]| format!(r#"{{"name":"{name}","parent":{parent},"translation_ue":[{},{},{}],"rotation_ue":[0,0,0,1],"scale_ue":[1,1,1]}}"#, t[0], t[1], t[2]);
        let bones = [bone("root", -1, [0.0, 0.0, 0.0]), bone("upper", 0, [0.0, 0.0, 100.0]), bone("lower", 1, [0.0, -30.0, 0.0]), bone("hand", 2, [0.0, -30.0, 0.0])];
        let skeleton = Skeleton::from_manifest(&Json::parse(&format!(r#"{{"skeleton":{{"bones":[{}]}}}}"#, bones.join(","))).unwrap()).unwrap();
        let at = |local: &[Trs], n: &str| component_space(&skeleton, local)[skeleton.index_of(n).unwrap()].translation;
        // a place within reach, in front and below
        let mut local = skeleton.reference_pose();
        let to = Vec3::new(30.0, -20.0, 80.0);
        reach_arm(&skeleton, 1, 2, 3, to, 1.0, &mut local);
        assert!(close(at(&local, "hand"), to), "{:?}", at(&local, "hand"));
        assert!(((at(&local, "lower") - at(&local, "upper")).length() - 30.0).abs() < 1.0e-3);
        let hand = component_space(&skeleton, &local)[3].rotation;
        assert!(hand.angle_between(Quat::IDENTITY) < 1.0e-3, "the hand keeps its rotation");
        // a place too far: the arm stretches toward it
        let mut local = skeleton.reference_pose();
        reach_arm(&skeleton, 1, 2, 3, Vec3::new(200.0, 0.0, 100.0), 1.0, &mut local);
        assert!(close(at(&local, "hand"), Vec3::new(59.94, 0.0, 100.0)), "{:?}", at(&local, "hand"));
        // weight 0 changes nothing
        let mut local = skeleton.reference_pose();
        reach_arm(&skeleton, 1, 2, 3, to, 0.0, &mut local);
        assert_eq!(local, skeleton.reference_pose());
    }

    #[test]
    fn a_bent_source_limb_bends_the_target_s() {
        let skeleton = biped();
        let retarget = Retarget::new(&skeleton, &ROWS, &FRAME, &source_bind).unwrap();
        let mut source = posed(Quat::IDENTITY, Vec3::ZERO);
        // the left arm raised forward (+X): a quarter turn about Y from hanging down (-Z -> ... ) : -Z to +X is about -Y
        let raise = Quat::from_rotation_arc(Vec3::NEG_Z, Vec3::X);
        source[5].1 = raise * source[5].1;
        let mut local = skeleton.reference_pose();
        retarget.apply(&skeleton, &source, Vec3::new(0.0, 0.0, 50.0), &Trs::IDENTITY, 1.0, &mut local);
        let c = component_space(&skeleton, &local);
        let at = |n: &str| c[skeleton.index_of(n).unwrap()].translation;
        assert!(close(at("l_hand") - at("l_arm"), Vec3::new(30.0, 0.0, 0.0)));
    }
}
