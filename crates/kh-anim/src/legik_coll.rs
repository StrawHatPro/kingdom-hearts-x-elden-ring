//! (party A2) `TresAnimNode_CollTwoBoneIK` (vtable 0x145D1CE20, ctor 0x14052ABE0): one leg per node, a shape swept
//! from above the foot to just below it, the stock two-bone solve onto the (shifted) foot, the foot turned toward the
//! surface normal. Donald's anim graph `n_ex001_Anim` runs two of them (R_ashi1 then L_ashi1) with no Multiped node;
//! Sora's two sit in the branch his graph does not take. Algorithm: `build/sora_tmp/native_D_legik.md` section 5
//! (EvaluateSkeletalControl 0x140536200 [C], Update 0x140555200 [C]); the collision rotation's operand order
//! (`Wrot * CollisionRotation`) and the shape sizes (`min(scale) * CollisionSize`) re-read for A2 in 0x140536200
//! (lines 585-616 and 805-816 of the decompiled C).
//!
//! Per evaluation (alpha > 1e-5):
//! ```text
//! E = CS transform of EffectorSpaceBoneName (Donald: the foot itself); W = E * C2W; s = min(C2W scale)
//! centre = Wpos + Wrot (Wscale * CollisionOffset) + (0, 0, IkOffset > 0 ? -IkOffset * s : 0)
//! S = C2W.rot (0, 0, CheckMax * scale.z); Eo = C2W.rot (0, 0, CheckMin * scale.z)
//! hit = IkSweep(centre + S -> centre + Eo, shape * s, Wrot * CollisionRotation)
//! EffectorLocation (bone space) = 0; on a hit: Wrot^-1 (Eo + up (PenetrationDepth + 0.01)) / Wscale
//! stock SolveTwoBoneIK(root, joint, foot; joint target, effector) -> root, joint, foot position
//! foot rotation: toward the hit normal, limited to m_MaintainEffectorLimitRot (axis-angle), faded in over
//!   StartInterpTime and out over EndInterpTime (linear weight, slerp)
//! stock LocalBlendCSBoneTransforms with the node's alpha (NOT capped at 0.99, unlike the Multiped node)
//! ```

use glam::{Quat, Vec3};

use crate::blend::{find_between, normalise, slerp_not_normalised, transform_blend, Rotator};
use crate::json::Json;
use crate::legik::{solve_two_bone_ik, IkShape, IkWorld};
use crate::pack::Skeleton;
use crate::Trs;

/// What one evaluation did (tests, debug text).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CollIkReport {
    pub alpha: f32,
    pub hit: bool,
    pub penetration_depth: f32,
    pub impact_normal: Vec3,
    /// The bone-space effector the solve used (0 without a hit).
    pub effector: Vec3,
    /// The weight of the hit-normal rotation (+0x130).
    pub normal_blend: f32,
}

#[derive(Clone, Debug)]
pub struct CollTwoBoneIk {
    foot: usize,
    joint: usize,
    root: usize,
    effector_bone: usize,
    joint_target: Vec3,
    joint_target_bone: usize,
    allow_stretching: bool,
    start_stretch_ratio: f32,
    max_stretch_scale: f32,
    maintain_hit_normal: bool,
    limit_rot: f32,
    start_interp: f32,
    end_interp: f32,
    /// 0 sphere, 1 capsule, 2 box (`ETresIkCollision`).
    shape: u8,
    size: Vec3,
    offset: Vec3,
    rotation: Quat,
    check_min: f32,
    check_max: f32,
    ik_offset: f32,
    // runtime (+0x120 stored normal quat, +0x130 blend, +0x134 previous ActualAlpha)
    stored: Quat,
    blend: f32,
    previous_alpha: f32,
    pub report: CollIkReport,
}

impl CollTwoBoneIk {
    /// Every `coll_two_bone_ik[].properties` of a `legik.json`, in graph evaluation order (the file's order is the
    /// node index order; `party_pack.js` writes them in evaluation order: R_ashi1 first).
    pub fn from_json_all(data: &Json, skeleton: &Skeleton) -> Result<Vec<CollTwoBoneIk>, String> {
        let nodes = data.get("coll_two_bone_ik").items();
        if nodes.is_empty() {
            return Err("legik.json has neither a Multiped nor a CollTwoBoneIK node".to_string());
        }
        nodes.iter().map(|n| CollTwoBoneIk::from_json(n.get("properties"), skeleton)).collect()
    }

    pub fn from_json(p: &Json, skeleton: &Skeleton) -> Result<CollTwoBoneIk, String> {
        let bone = |name: &Json| -> Result<usize, String> {
            let name = name.as_str().ok_or("CollTwoBoneIK: a bone name is missing")?;
            skeleton.index_of(name).ok_or(format!("CollTwoBoneIK: the skeleton has no bone {name}"))
        };
        let f = |key: &str, default: f32| p.get(key).as_f32().unwrap_or(default);
        let v = |o: &Json| Vec3::new(o.get("x").as_f32().unwrap_or(0.0), o.get("y").as_f32().unwrap_or(0.0), o.get("z").as_f32().unwrap_or(0.0));
        let foot = bone(p.get("IKBone").get("BoneName"))?;
        let joint = skeleton.bones[foot].parent.ok_or("CollTwoBoneIK: the foot has no parent")?;
        let root = skeleton.bones[joint].parent.ok_or("CollTwoBoneIK: the shin has no parent")?;
        // The data's spaces: Donald's (and Sora's) are bone space; the world / component paths are not modelled.
        if p.get("EffectorLocationSpace").as_str() != Some("BCS_BoneSpace") || p.get("JointTargetLocationSpace").as_str() != Some("BCS_BoneSpace") {
            return Err("CollTwoBoneIK: only bone-space effector / joint target are modelled".to_string());
        }
        if p.get("bTakeRotationFromEffectorSpace").as_bool() == Some(true) || p.get("bMaintainEffectorRelRot").as_bool() == Some(true) {
            return Err("CollTwoBoneIK: bTakeRotationFromEffectorSpace / bMaintainEffectorRelRot are not modelled".to_string());
        }
        let shape = match p.get("CollisionShape").as_str() {
            Some("ETresIkCollision::SPHERE") | None => 0,
            Some("ETresIkCollision::CAPSULE") => 1,
            Some("ETresIkCollision::BOX") => 2,
            Some(other) => return Err(format!("CollTwoBoneIK: shape {other}")),
        };
        let r = p.get("CollisionRotation");
        let rotation = Rotator { pitch: r.get("pitch").as_f32().unwrap_or(0.0), yaw: r.get("yaw").as_f32().unwrap_or(0.0), roll: r.get("roll").as_f32().unwrap_or(0.0) }.quat();
        let effector_bone = p.get("EffectorSpaceBoneName").as_str().and_then(|n| skeleton.index_of(n)).unwrap_or(joint);
        Ok(CollTwoBoneIk {
            foot,
            joint,
            root,
            effector_bone,
            joint_target: v(p.get("JointTargetLocation")),
            joint_target_bone: bone(p.get("JointTargetSpaceBoneName"))?,
            allow_stretching: p.get("bAllowStretching").as_bool().unwrap_or(false),
            start_stretch_ratio: f("StartStretchRatio", 1.0),
            max_stretch_scale: f("MaxStretchScale", 1.2),
            maintain_hit_normal: p.get("m_bMaintainEffectorHitNormal").as_bool().unwrap_or(false),
            limit_rot: f("m_MaintainEffectorLimitRot", 25.0),
            start_interp: f("m_MaintainEffectorStartInterpTime", 0.1),
            end_interp: f("m_MaintainEffectorEndInterpTime", 0.2),
            shape,
            size: if p.get("CollisionSize").is_null() { Vec3::ONE } else { v(p.get("CollisionSize")) },
            offset: v(p.get("CollisionOffset")),
            rotation,
            check_min: f("CheckMin", 0.0),
            check_max: f("CheckMax", 30.0),
            ik_offset: f("IkOffset", 0.0),
            stored: Quat::IDENTITY,
            blend: 0.0,
            previous_alpha: 0.0,
            report: CollIkReport::default(),
        })
    }

    /// thigh, shin, foot.
    pub fn bones(&self) -> [usize; 3] {
        [self.root, self.joint, self.foot]
    }

    /// One update + evaluation of the node. `local` / `component` = the node's input pose (the previous node's
    /// output); `local` receives the result.
    #[allow(clippy::too_many_arguments)]
    pub fn evaluate(&mut self, skeleton: &Skeleton, local: &mut [Trs], component: &[Trs], alpha: f32, dt: f32, c2w: &Trs, world: &dyn IkWorld) {
        self.report = CollIkReport::default();
        // Update 0x140555200: stock (ActualAlpha = AlphaScaleBias (1, 0) applied, clamped), then the jump flag.
        let actual = alpha.clamp(0.0, 1.0);
        let jumped = self.previous_alpha <= 1.0e-5 && actual >= 0.99999;
        self.previous_alpha = actual;
        if !(actual > 1.0e-5) {
            return;
        }
        self.report.alpha = actual;

        let e = component[self.effector_bone];
        let w = e.then(c2w);
        let s = c2w.scale.x.min(c2w.scale.y).min(c2w.scale.z);
        let centre = w.translation + w.rotation * (w.scale * self.offset) + Vec3::new(0.0, 0.0, if self.ik_offset > 0.0 { -self.ik_offset * s } else { 0.0 });
        let start = c2w.rotation * Vec3::new(0.0, 0.0, self.check_max * c2w.scale.z);
        let end = c2w.rotation * Vec3::new(0.0, 0.0, self.check_min * c2w.scale.z);
        let size = self.size * s;
        let shape = match self.shape {
            0 => IkShape::Sphere(size.x),
            1 => IkShape::Capsule { radius: size.x, half_height: size.z },
            _ => IkShape::Box(size),
        };
        let mut effector = Vec3::ZERO;
        let mut normal = Vec3::ZERO;
        let mut hit = false;
        if let Some(found) = world.sweep_shape(centre + start, centre + end, shape, w.rotation * self.rotation) {
            hit = true;
            let d = end + Vec3::Z * (found.penetration_depth + 0.01);
            let one = |x: f32| if x.abs() <= 1.0e-8 { 0.0 } else { 1.0 / x };
            effector = (w.rotation.conjugate() * d) * Vec3::new(one(w.scale.x), one(w.scale.y), one(w.scale.z));
            normal = found.hit.impact_normal;
            self.report.penetration_depth = found.penetration_depth;
        }
        self.report.hit = hit;
        self.report.effector = effector;
        self.report.impact_normal = normal;

        // ConvertBoneSpaceTransformToCS (bone space): the bone's CS transform applied to the location.
        let effector_cs = e.transform_point(effector);
        let joint_target = component[self.joint_target_bone].transform_point(self.joint_target);
        let (mut r, mut j, mut f) = (component[self.root], component[self.joint], component[self.foot]);
        solve_two_bone_ik(&mut r, &mut j, &mut f, joint_target, effector_cs, self.allow_stretching, self.start_stretch_ratio, self.max_stretch_scale);

        let mut end_rotation = f.rotation;
        if self.maintain_hit_normal {
            if !hit || normal.length_squared() <= 0.0 {
                if self.blend > 0.0 {
                    self.blend = if self.end_interp <= 0.0 { 0.0 } else { (self.blend - dt / self.end_interp).clamp(0.0, 1.0) };
                    if self.blend > 0.0 {
                        end_rotation = normalise(slerp_not_normalised(end_rotation, self.stored * end_rotation, self.blend));
                    }
                }
            } else {
                let one = |x: f32| if x.abs() <= 1.0e-8 { 0.0 } else { 1.0 / x };
                let n = (c2w.rotation.conjugate() * normal) * Vec3::new(one(c2w.scale.x), one(c2w.scale.y), one(c2w.scale.z));
                let mut q = find_between(Vec3::Z, n);
                if self.limit_rot > 0.0 {
                    let half = q.w.clamp(-1.0, 1.0).acos();
                    let sn = (1.0 - q.w * q.w).max(0.0).sqrt();
                    let axis = if sn < 1.0e-4 { Vec3::X } else { Vec3::new(q.x, q.y, q.z) / sn };
                    let angle = (2.0 * half).min(self.limit_rot * 0.017_453_292);
                    let (sa, ca) = (angle * 0.5).sin_cos();
                    q = normalise(Quat::from_xyzw(axis.x * sa, axis.y * sa, axis.z * sa, ca));
                }
                if self.blend < 1.0 {
                    self.blend = if self.start_interp <= 0.0 || jumped { 1.0 } else { (self.blend + dt / self.start_interp).clamp(0.0, 1.0) };
                }
                self.stored = q;
                let full = normalise(q * end_rotation);
                end_rotation = if self.blend < 1.0 { normalise(slerp_not_normalised(end_rotation, full, self.blend)) } else { full };
            }
        }
        self.report.normal_blend = self.blend;
        let out = [(self.root, r), (self.joint, j), (self.foot, Trs { rotation: end_rotation, translation: f.translation, scale: f.scale })];
        local_blend(skeleton, local, component, &out, actual);
    }
}

/// Stock `FCSPose::LocalBlendCSBoneTransforms` as `legik.rs` reads it (4.17): the list sorted by bone index; each entry
/// made local against its parent's NEW transform when the parent is the entry just before it, else against the
/// parent's input transform; blended in local space by `alpha`.
fn local_blend(skeleton: &Skeleton, local: &mut [Trs], component: &[Trs], out: &[(usize, Trs)], alpha: f32) {
    let mut out = out.to_vec();
    out.sort_by_key(|(bone, _)| *bone);
    for i in 0..out.len() {
        let (bone, transform) = out[i];
        let target = match skeleton.bones[bone].parent {
            Some(parent) if i > 0 && out[i - 1].0 == parent => transform.relative_to(&out[i - 1].1),
            Some(parent) => transform.relative_to(&component[parent]),
            None => transform,
        };
        local[bone] = transform_blend(&local[bone], &target, alpha.clamp(0.0, 1.0));
    }
}
