//! (ANIMS 2026-10-23) THE BORROWED ELDEN RING ACTIONS AT EACH CHARACTER'S OWN PROPORTIONS.
//!
//! THE USER: "please fix ladder animations for all the characters, and adapt all of the borrowed Elden Ring animations
//! that Sora uses for Donald and Goofy properly, respecting their differences in proportion. This includes opening doors
//! and walking through fog gates". (LADDER2 2026-10-25: nobody climbs a ladder any more, the user's "jump up the entire
//! height of the ladder": the ladder parts of this fit (the rungs, the alignment square to the ladder) are gone.)
//!
//! WHAT IT WAS (figure.rs / kh-anim `Retarget`, sora.rs `step_pose`, party/leader.rs `PoseOver`): every bone turned as
//! the Tarnished's, the pelvis put at the Tarnished's pelvis with only its HEIGHT scaled by the leg ratio (Donald's legs
//! are 0.34 of the Tarnished's, Goofy's longer), each hand sent to the Tarnished's hand "as far as the arm reaches", on a
//! ladder each foot to the Tarnished's foot and the "rung rule" (a rung out of reach -> 0.5 m lower / higher). So:
//! - the pelvis swung, leaned and stepped at the TARNISHED'S scale around Donald's tiny legs (his feet skated);
//! - Donald stood where the Tarnished stands, 30 - 50 cm from a door or a ladder his 30 cm arms cannot span: his hands
//!   hung in the air short of the rung / the handle; Goofy's long arms folded up against the ladder, knees through it;
//! - the rung rule only ever moved a limb when it was OUT of reach, never toward a rung at the limb's natural stretch:
//!   Donald's knees came up to his chin for the Tarnished's next rung;
//! - the ankle was put where the Tarnished's ankle is (its sole 11 cm under it): Donald's and Goofy's soles floated or sank;
//! - the walk inside an action (the fog's 1.8 m, a door's steps) moved his root at the Tarnished's pace with legs a third
//!   as long: the feet slid;
//! - his posture was the Tarnished's (upright knight), not Donald's or Goofy's.
//!
//! WHAT IT IS (`Fit::plan` once a frame, pure; `apply` where the pose is made):
//! - THE PELVIS: its height above the floor AND its offset from the character's own root scaled by the leg ratio (the
//!   lean, the sway, a step of the clip at his size), then moved by the STANDOFF;
//! - THE STANDOFF (a door, a lever, the fog: actions with contacts): the whole figure is moved toward / away from what
//!   the hands touch until each arm is as stretched as the Tarnished's is (its share of its reach), never closer to the
//!   contact plane than his body's depth (no belly / knee through the door), smoothed (`TAU_S`, frame-rate independent);
//! - THE FEET (Donald, Goofy): planted where they stand while the Tarnished's foot is planted (no sliding),
//!   stepping anew when the root has moved past his own stride (short legs: more, quicker steps), lifted on the way;
//! - THE STYLE: his spine, neck and head keep a share of his own pose (`Profile::style_keep`: Donald's stoop, Goofy's
//!   slouch), the arms and legs do the action.

use glam::{Quat, Vec3};

/// The `ROWS` (figure.rs) rows used here.
pub mod rows {
    pub const PELVIS: usize = 0;
    pub const ARM_L: [usize; 3] = [7, 8, 9];
    pub const ARM_R: [usize; 3] = [11, 12, 13];
    pub const LEG_L: [usize; 3] = [14, 15, 16];
    pub const LEG_R: [usize; 3] = [18, 19, 20];
    pub const TOE_L: usize = 17;
    pub const TOE_R: usize = 21;
    /// The limbs in the order of limbs.rs `LIMBS`: left hand, right hand, left foot, right foot.
    pub const LIMBS: [[usize; 3]; 4] = [ARM_L, ARM_R, LEG_L, LEG_R];
}

/// A body's measures (cm), from its reference / bind pose.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Dims {
    /// Pelvis (the retarget's root) above the floor.
    pub pelvis_h: f32,
    /// The ankle (the foot bone) above the floor.
    pub ankle_h: f32,
    /// The hips (the thighs' roots) above the floor.
    pub hip_h: f32,
    /// Half the width between the hips.
    pub hip_w: f32,
    /// (ANIMS) The top of his head above the floor (the head bone plus most of the chest-to-head span; Donald's big head).
    pub head_h: f32,
    /// (ANIMS) His shoulders (the upper arms' roots) above the floor, and half their width.
    pub shoulder_h: f32,
    pub shoulder_w: f32,
    /// Limb lengths (upper + lower), in limbs.rs order.
    pub limb: [f32; 4],
    /// Pelvis to each limb's root (shoulder / hip).
    pub root: [f32; 4],
}

impl Dims {
    /// From the positions of the pelvis, each limb's three joints (limbs.rs order) and the floor's height.
    pub fn from_points(pelvis: Vec3, limbs: [[Vec3; 3]; 4], floor: f32) -> Option<Dims> {
        let mut limb = [0.0; 4];
        let mut root = [0.0; 4];
        for k in 0..4 {
            let [a, b, c] = limbs[k];
            limb[k] = (b - a).length() + (c - b).length();
            root[k] = (a - pelvis).length();
        }
        let ankle_h = ((limbs[2][2].z + limbs[3][2].z) * 0.5 - floor).max(0.0);
        let hip_h = (limbs[2][0].z + limbs[3][0].z) * 0.5 - floor;
        let hip_w = (limbs[2][0] - limbs[3][0]).length() * 0.5;
        let d = Dims { pelvis_h: pelvis.z - floor, ankle_h, hip_h, hip_w, head_h: (pelvis.z - floor) * 1.8, shoulder_h: (limbs[0][0].z + limbs[1][0].z) * 0.5 - floor, shoulder_w: (limbs[0][0] - limbs[1][0]).length() * 0.5, limb, root };
        (d.pelvis_h > 1.0 && d.limb.iter().all(|l| *l > 1.0) && d.root.iter().all(|r| r.is_finite())).then_some(d)
    }

    /// The Tarnished's (or any `ROWS` source's) from its bind pose (KH3 axes, cm): the floor at its toes.
    pub fn of_binds(binds: &[(Vec3, Quat)]) -> Option<Dims> {
        if binds.len() < 22 {
            return None;
        }
        let p = |k: usize| binds[k].0;
        let floor = p(rows::TOE_L).z.min(p(rows::TOE_R).z);
        let mut d = Dims::from_points(p(rows::PELVIS), rows::LIMBS.map(|l| l.map(p)), floor)?;
        d.head_h = head_top(p(5), p(3)) - floor;
        Some(d)
    }

    /// A KH3 skeleton's (Sora, Donald, Goofy: the same bone names) from its reference pose: the floor at its toes.
    pub fn of_skeleton(s: &kh_anim::Skeleton) -> Option<Dims> {
        let c = kh_anim::pose::component_space(s, &s.reference_pose());
        let at = |n: &str| s.index_of(n).map(|k| c[k].translation);
        let names = crate::limbs::LIMBS;
        let mut limbs = [[Vec3::ZERO; 3]; 4];
        for k in 0..4 {
            for j in 0..3 {
                limbs[k][j] = at(names[k][j])?;
            }
        }
        let floor = at("L_ashi2")?.z.min(at("R_ashi2")?.z).min(limbs[2][2].z);
        let mut d = Dims::from_points(at("center")?, limbs, floor)?;
        if let (Some(head), Some(chest)) = (at("atama"), at("mune")) {
            d.head_h = head_top(head, chest) - floor;
        }
        Some(d)
    }
}

/// (ANIMS) The top of a head from its head bone and its chest bone (the head bone is the skull's pivot).
pub fn head_top(head: Vec3, chest: Vec3) -> f32 {
    head.z + (0.6 * (head - chest).length()).max(8.0)
}

/// Each character's way of doing it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Profile {
    /// The share of his own spine / neck / head pose kept over the Tarnished's (0 = the Tarnished's).
    pub style_keep: f32,
    /// His body's depth in front of the pelvis (cm): the contact plane is never nearer.
    pub depth: f32,
    /// His feet are planted / stepped (all three: Sora's too, his leg IK then only meets the floor).
    pub plant_feet: bool,
    /// His hips are never higher over his ankles than this share of his leg (the Tarnished stands straight-legged; his
    /// knees keep a bend, so that a planted foot has room to stay while he moves: Donald's 19 cm legs most of all).
    pub hip_cap: f32,
    /// (ANIMS) His neck and head keep this share of his own pose in every borrowed action (Donald's and Goofy's big heads on
    /// the Tarnished's neck turns read as a head tucked into the shoulders: the user's fog gate).
    pub head_keep: f32,
    /// (ANIMS) A hand that cannot touch what the Tarnished's touches is put where HIS arm would point, no higher than this
    /// over his shoulder (degrees): no "hands up" at a fog wall.
    pub max_arm_lift: f32,
}

impl Profile {
    pub const SORA: Profile = Profile { style_keep: 0.0, depth: 16.0, plant_feet: true, hip_cap: 0.97, head_keep: 0.0, max_arm_lift: 35.0 };
    /// Donald: short, big belly, a forward stoop (n_ex001).
    pub const DONALD: Profile = Profile { style_keep: 0.45, depth: 20.0, plant_feet: true, hip_cap: 0.88, head_keep: 1.0, max_arm_lift: 30.0 };
    /// Goofy: tall, thin, a slouch (n_ex002).
    pub const GOOFY: Profile = Profile { style_keep: 0.3, depth: 15.0, plant_feet: true, hip_cap: 0.92, head_keep: 1.0, max_arm_lift: 35.0 };

    pub fn of(chara: kh_host::party::PartyChara) -> Profile {
        match chara {
            kh_host::party::PartyChara::Sora => Profile::SORA,
            kh_host::party::PartyChara::Donald => Profile::DONALD,
            kh_host::party::PartyChara::Goofy => Profile::GOOFY,
        }
    }
}

/// What the action touches.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    /// A door, a lever, the fog, a chest: the hands on something in front.
    Surface,
    /// Nothing in the world (the grace, a grab): the hands where the Tarnished's are, as far as they reach.
    Free,
}

/// The plan of a frame (world, KH3 cm).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Plan {
    pub pelvis: Vec3,
    /// Each limb's end (limbs.rs order); None = left as the retarget turned it.
    pub limbs: [Option<Vec3>; 4],
    pub style_keep: f32,
    /// The style is kept on the neck and head only (an action that reaches: the spine bends as the Tarnished's).
    pub head_only: bool,
    /// (ANIMS) His neck / head kept from his own pose (`Profile::head_keep`).
    pub head_keep: f32,
    /// (ANIMS) His own spine kept whole (`upright`: he reaches up to what the Tarnished bows down to).
    pub spine_keep: f32,
    /// The standoff applied (diagnostics).
    pub shift: Vec3,
    /// (ANIMS) THE POSE SANITY (`sanity`): false = this plan would fold or over-stretch him: show his own pose instead.
    pub sane: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Foot {
    locked: Option<Vec3>,
    /// (from, seconds into the swing)
    swing: Option<(Vec3, f32)>,
    last_src: Option<Vec3>,
    out: Option<Vec3>,
}

/// The state of one character's fit (the smoothing, the planted feet).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Fit {
    shift: Vec3,
    /// The crouch (cm), smoothed as the standoff.
    drop: f32,
    ready: bool,
    feet: [Foot; 2],
    kind: Option<Kind>,
    last_root: Option<Vec3>,
    root_vel: Vec3,
}

impl Fit {
    /// The standoff's smoothing (s).
    pub const TAU_S: f32 = 0.12;
    /// A foot's swing to its new place (s).
    pub const SWING_S: f32 = 0.18;
    pub const SWING_MIN_S: f32 = 0.1;
    /// A source foot slower than this (cm/s) and this near the floor (cm) is planted.
    pub const PLANT_SPEED: f32 = 30.0;
    pub const PLANT_LIFT: f32 = 6.0;
    /// A planted foot steps anew when its place is this share of the leg from where his stride would put it.
    pub const STEP_SHARE: f32 = 0.45;
    /// The standoff's limits (cm): toward, away.
    pub const MAX_IN: f32 = 45.0;
    pub const MAX_OUT: f32 = 35.0;
    /// (ANIMS) A real contact (a lid, a handle) out of reach is still stretched for while it is no steeper than this over
    /// his shoulder (degrees); steeper (the fog's parting arms at the Tarnished's head height) his own arm's range instead.
    pub const CONTACT_LIFT_DEG: f32 = 50.0;

    pub fn reset(&mut self) {
        *self = Fit::default();
    }

    /// ONE FRAME. `src`: every `ROWS` row of the Tarnished's pose (world); `s` / `t`: the Tarnished's / his measures;
    /// `ratio`: his legs / the Tarnished's (the retarget's); `root`: his feet's place (x, y; z unused); `floor`: his floor's
    /// height; `dt`: seconds.
    #[allow(clippy::too_many_arguments)]
    pub fn plan(&mut self, src: &[(Vec3, Quat)], s: &Dims, t: &Dims, ratio: f32, prof: &Profile, root: Vec3, floor: f32, kind: Kind, dt: f32) -> Plan {
        let p = |k: usize| src.get(k).map_or(Vec3::ZERO, |r| r.0);
        if self.kind.as_ref().map(std::mem::discriminant) != Some(std::mem::discriminant(&kind)) {
            // (a new action: nothing of the last one is carried)
            self.feet = Default::default();
            self.kind = Some(kind);
        }
        // (ANIMS) HIS LEGS / THE TARNISHED'S, AS CHAINS (thigh + shin): the retarget's ratio measures the pelvis bone's height
        // over the ankle, which for Sora (his `center` 12 cm over his hips) and Donald overstates the legs: the pelvis went so
        // high that his feet could not reach the body's floor (Sora 3 cm, Donald 10 cm)
        let chain = t.limb[2] / s.limb[2].max(1.0);
        let ratio = if chain.is_finite() && chain > 0.05 { chain } else if ratio.is_finite() && ratio > 0.05 { ratio } else { 1.0 };
        let sp = p(rows::PELVIS);
        let flat = |v: Vec3| Vec3::new(v.x, v.y, 0.0);
        // THE PELVIS at his size: its height above the floor and its offset from his root, scaled
        // (the hips over the ankles scaled, the pelvis bone at his own height over his hips)
        let hips_over_ankles = sp.z - (s.pelvis_h - s.hip_h) - floor - s.ankle_h;
        let hips = (hips_over_ankles * ratio).min(prof.hip_cap * t.limb[2]);
        // (a squat at his size keeps his knees above the floor: short legs fold no lower than this; a sit is a sit)
        let sitting = hips_over_ankles < 0.27 * s.limb[2];
        let hips = if sitting { hips } else { hips.max(0.42 * t.limb[2]) };
        let base = Vec3::new(root.x, root.y, 0.0) + flat(sp - root) * ratio + Vec3::Z * (floor + t.ankle_h + hips + (t.pelvis_h - t.hip_h));
        // his limbs' roots, estimated from the Tarnished's (their direction) at his length
        let limb_root = |k: usize, pelvis: Vec3| {
            let a = p(rows::LIMBS[k][0]);
            pelvis + (a - sp).normalize_or_zero() * t.root[k]
        };
        let stretch = |k: usize| {
            let [a, _, c] = rows::LIMBS[k].map(p);
            ((c - a).length() / s.limb[k].max(1.0)).clamp(0.35, 0.92)
        };
        // the hands' goals: where the Tarnished's are
        let mut targets: [Option<Vec3>; 4] = [None; 4];
        for k in 0..2 {
            targets[k] = Some(p(rows::LIMBS[k][2]));
        }
        // (ANIMS) UPRIGHT: what the Tarnished bends DOWN to (a chest lid at its waist) is at HIS shoulder or head: he does not
        // copy the bow, he stands as he stands (his own spine) and reaches from his upright shoulders
        let across_src = flat(p(rows::LEG_L[0]) - p(rows::LEG_R[0])).normalize_or_zero();
        let upright = matches!(kind, Kind::Surface)
            && (0..2).any(|k| targets[k].is_some_and(|c| c.z < p(rows::LIMBS[k][0]).z - 5.0 && c.z > floor + t.shoulder_h - 0.25 * t.limb[k]));
        let hand_root = |k: usize, pelvis: Vec3| {
            if upright && k < 2 {
                pelvis + Vec3::Z * (t.shoulder_h - t.pelvis_h) + across_src * t.shoulder_w * if k == 0 { 1.0 } else { -1.0 }
            } else {
                limb_root(k, pelvis)
            }
        };
        // (ANIMS) WHAT IS ABOVE HIS REACH IS TOUCHED AT HIS HEIGHT: a door, the fog, a post the Tarnished touches at its chest is
        // touched by Donald where his arms come, on the same spot of it (its place across and in front, his height)
        if matches!(kind, Kind::Surface) {
            let lift = prof.max_arm_lift.to_radians().sin();
            for k in 0..2 {
                let Some(c) = targets[k] else { continue };
                let a = hand_root(k, base);
                let top = (a.z + lift * t.limb[k]).min(base.z + (t.head_h - t.pelvis_h));
                if c.z > top && flat(c - sp).dot(Vec3::Z.cross(across_src)) > 0.25 * s.limb[k] {
                    targets[k] = Some(Vec3::new(c.x, c.y, top));
                }
            }
        }
        // THE STANDOFF: toward / away from the contacts until each arm has the Tarnished's stretch
        let want_shift = match kind {
            Kind::Free => Vec3::ZERO,
            _ => {
                let mut sum = 0.0;
                let mut n = 0.0;
                let mut plane = f32::MAX;
                // (only forward, the body's facing; only the hands that reach out in front: a hand hanging at the side is no contact)
                let forward = Vec3::Z.cross(flat(p(rows::LEG_L[0]) - p(rows::LEG_R[0]))).normalize_or_zero();
                let contact = |k: usize| targets[k].filter(|c| flat(*c - sp).dot(forward) > 0.25 * s.limb[k]);
                for k in 0..2 {
                    let Some(c) = contact(k) else { continue };
                    let a = hand_root(k, base);
                    let v = c - a;
                    // (ANIMS 2026-10-23, the user's Donald at a fog gate: "hunched far forward, one leg kicked out straight behind") a
                    // contact far above or below his reach is out of it however near he stands: walking his body under it only tipped
                    // him over his planted feet. Only contacts his arm can span vertically pull him.
                    if v.z.abs() > 1.2 * t.limb[k] {
                        continue;
                    }
                    let ahead = flat(v).dot(forward);
                    let side = (flat(v) - forward * ahead).length();
                    let d = stretch(k) * t.limb[k];
                    let rest = d * d - v.z * v.z - side * side;
                    sum += if rest <= 0.0 { ahead } else { ahead - rest.sqrt() };
                    n += 1.0;
                }
                match (n > 0.0 && forward != Vec3::ZERO).then_some(forward) {
                    Some(u) => {
                        // (a wall: only what the hands touch above his hips; an item on the ground is no wall, his feet go by it)
                        for k in 0..2 {
                            if let Some(c) = contact(k).filter(|c| c.z > floor + t.hip_h) {
                                plane = plane.min(flat(c - base).dot(u));
                            }
                        }
                        // (nor his feet: the stance he takes, at his size, and his toes)
                        let toes = 0.15 * t.limb[2] + 4.0;
                        let front = (2..4).map(|k| flat(p(rows::LIMBS[k][2]) - root).dot(u) * ratio).fold(f32::MIN, f32::max);
                        let foot_rel = flat(Vec3::new(root.x, root.y, 0.0) - base).dot(u) + front;
                        // (nor his knees, at a door or a wall: the Tarnished's knee before its hip, at his size)
                        let knee = (2..4).map(|k| flat(p(rows::LIMBS[k][1]) - p(rows::LIMBS[k][0])).dot(u) * ratio + flat(limb_root(k, base) - base).dot(u)).fold(f32::MIN, f32::max);
                        let room = if plane < f32::MAX { (plane - prof.depth).min(plane - foot_rel - toes).min(plane - knee - 3.0) } else { Self::MAX_IN };
                        // (the limits at his size: Donald moves a few centimetres, not 45)
                        let (max_in, max_out) = (Self::MAX_IN.min(0.5 * (t.limb[0] + t.limb[2])), Self::MAX_OUT.min(0.4 * (t.limb[0] + t.limb[2])));
                        let s_avg = (sum / n).clamp(-max_out, max_in).min(room.max(-max_out));
                        u * s_avg
                    }
                    None => Vec3::ZERO,
                }
            }
        };
        let follow = if self.ready && dt > 0.0 { 1.0 - (-dt / Self::TAU_S).exp() } else { 1.0 };
        self.shift += (want_shift - self.shift) * follow;
        // THE CROUCH: a hand's contact below his reach (an item on the ground, a low lid) lowers his hips, knees bent, at most
        // by half his hips' height
        let want_drop = match kind {
            Kind::Surface => (0..2)
                .filter_map(|k| {
                    let c = targets[k]?;
                    let a = hand_root(k, base + self.shift);
                    let (h, vz, reach) = (flat(c - a).length(), a.z - c.z, 0.95 * t.limb[k]);
                    (vz > 0.0 && (c - a).length() > reach).then(|| if reach > h { vz - (reach * reach - h * h).sqrt() } else { vz })
                })
                .fold(0.0f32, f32::max)
                .min(0.5 * (t.hip_h - t.ankle_h).max(0.0))
                // (never so low that his knees would have to go through the floor)
                .min((base.z - (t.pelvis_h - t.hip_h) - (floor + t.ankle_h + 0.4 * t.limb[2])).max(0.0)),
            _ => 0.0,
        };
        self.drop += (want_drop - self.drop) * follow;
        self.ready = true;
        let pelvis = base + self.shift - Vec3::Z * self.drop;
        // (ANIMS) HIS HANDS AT HIS HEIGHT: what the Tarnished touches within his reach stays the goal; else the
        // Tarnished's arm as a direction and share of its length from HIS shoulder, never lifted more than `max_arm_lift` over
        // his shoulder (the fog gate's parting arms were Donald's hands up by his head)
        for k in 0..2 {
            let Some(c) = targets[k] else { continue };
            let a = hand_root(k, pelvis);
            let to_c = c - a;
            // (never over the top of his head: the fog's parting arms are at the Tarnished's head height, above Sora's)
            let head_top_z = pelvis.z + (t.head_h - t.pelvis_h);
            let under_head = c.z <= head_top_z;
            if to_c.length() <= 1.15 * t.limb[k] && under_head {
                continue;
            }
            // (a contact a little out of reach and not above his arm's lift: he stretches toward it, as anyone would)
            if under_head && to_c.length() <= 2.5 * t.limb[k] && to_c.z / to_c.length() <= Self::CONTACT_LIFT_DEG.to_radians().sin() {
                continue;
            }
            let src_arm = c - p(rows::LIMBS[k][0]);
            let mut v = src_arm * (t.limb[k] / s.limb[k].max(1.0));
            let len = v.length();
            let lift = prof.max_arm_lift.to_radians();
            if len > 1e-3 && v.z / len > lift.sin() {
                let h = flat(v).try_normalize().unwrap_or_else(|| Vec3::Z.cross(flat(p(rows::LEG_L[0]) - p(rows::LEG_R[0]))).normalize_or_zero());
                v = h * len * lift.cos() + Vec3::Z * len * lift.sin();
            }
            targets[k] = Some(a + v);
        }
        // THE FEET: planted, stepping anew past his stride
        if prof.plant_feet {
            // his root's speed (the body walks him through the fog, a door): the steps lead by it
            let vel = match (self.last_root, dt > 0.0) {
                (Some(l), true) => flat(root - l) / dt,
                _ => Vec3::ZERO,
            };
            self.root_vel += (vel - self.root_vel) * if dt > 0.0 { 1.0 - (-dt / 0.1).exp() } else { 1.0 };
            let speed = self.root_vel.length();
            let ahead = self.root_vel.try_normalize().unwrap_or(Vec3::ZERO);
            let other_swinging = [self.feet[1].swing.is_some(), self.feet[0].swing.is_some()];
            for j in 0..2 {
                let k = 2 + j;
                let sf = p(rows::LIMBS[k][2]);
                let lift = (sf.z - floor - s.ankle_h).max(0.0);
                let ground = floor + t.ankle_h;
                // (from HIS hip, at his stance: the Tarnished's foot from its hip, scaled; his hips are as wide as they are)
                let across = flat(p(rows::LEG_L[0]) - p(rows::LEG_R[0])).normalize_or_zero();
                let hip = flat(pelvis) + across * t.hip_w * if j == 0 { 1.0 } else { -1.0 };
                let stance = flat(sf - p(rows::LIMBS[k][0])) * ratio;
                // (walking: the fore-and-aft of his steps is his own, led by the root; only the Tarnished's stance width is kept)
                let stance = if speed >= 10.0 { stance - ahead * stance.dot(ahead) } else { stance };
                let desired = hip + stance + Vec3::Z * ground;
                // (a step before the planted leg runs out of reach: its room over the ground at the hips' height)
                let hip_z = pelvis.z - (t.pelvis_h - t.hip_h);
                let room = ((0.97 * t.limb[k]).powi(2) - (hip_z - ground).powi(2)).max(0.0).sqrt();
                let limit = (Self::STEP_SHARE * t.limb[k]).min(0.65 * room).max(1.0);
                // (the Tarnished raises this foot on purpose: a step up, a kick: his follows it at his size)
                // (only standing: walking, the steps are his own, at his stride)
                let raised = lift >= Self::PLANT_LIFT * 2.0 && speed < 10.0;
                let f = &mut self.feet[j];
                f.last_src = Some(sf);
                let out = if let Some((from, time)) = f.swing {
                    let swing_s = (1.2 * limit / speed.max(1.0)).clamp(Self::SWING_MIN_S, Self::SWING_S);
                    let time = time + dt;
                    let a = (time / swing_s).clamp(0.0, 1.0);
                    let e = a * a * (3.0 - 2.0 * a);
                    // the landing: where his stride puts the foot, led by the root's speed
                    let land = desired + ahead * (0.6 * limit).min(speed * swing_s) + self.root_vel * (swing_s - time).max(0.0);
                    let mut at = from.lerp(Vec3::new(land.x, land.y, ground), e);
                    at.z += (1.0 - (2.0 * a - 1.0).powi(4)) * 0.18 * t.limb[k]; // (up at once, down at the end: no scuff)
                    if a >= 1.0 {
                        f.swing = None;
                        f.locked = (!raised).then_some(Vec3::new(land.x, land.y, ground));
                    } else {
                        f.swing = Some((from, time));
                    }
                    at
                } else if raised {
                    f.locked = None;
                    desired + Vec3::Z * lift * ratio
                } else {
                    match f.locked {
                        Some(l) if flat(l - desired).length() > limit && (!other_swinging[j] || flat(l - desired).length() > 1.5 * limit) => {
                            f.swing = Some((l, 0.0));
                            f.locked = None;
                            l
                        }
                        Some(l) => l,
                        None => {
                            let at = f.out.unwrap_or(desired);
                            if flat(at - desired).length() > 1.0 || (at.z - ground).abs() > 1.0 {
                                f.swing = Some((at, 0.0));
                                at
                            } else {
                                f.locked = Some(desired);
                                desired
                            }
                        }
                    }
                };
                f.out = Some(out);
                targets[k] = Some(out);
            }
        } else {
            self.feet = Default::default();
        }
        self.last_root = Some(root);
        let sane = sanity(&targets, pelvis, t, floor, flat(p(rows::LEG_L[0]) - p(rows::LEG_R[0])).normalize_or_zero());
        Plan { pelvis, limbs: targets, style_keep: prof.style_keep, head_only: !matches!(kind, Kind::Free), head_keep: prof.head_keep, spine_keep: if upright { 1.0 } else { 0.0 }, shift: self.shift, sane }
    }

    /// A foot (0 left, 1 right) is swinging to a new place now (the proof's measures).
    pub fn swinging(&self, j: usize) -> bool {
        self.feet.get(j).is_some_and(|f| f.swing.is_some())
    }
}

// ---------------------------------------------------------------- (ANIMS) THE GUARDS (the user's Donald at a fog gate)
//
// The user, after the first build (screens 20261023231341/42): "Also Donald still looks fucked going through a fog gate, fix
// this!" Donald hunched far forward, one leg kicked straight out behind him, his staff on the ground. CAUSE (the plan's
// rules, read again against that picture; the session's own log held no fog walk): the fog walk's forward hand is at the
// Tarnished's chest height, 130 cm, far above Donald's 25 cm arm from a 50 cm shoulder. The standoff walked his whole body
// up to 45 cm toward it (more than twice his 19 cm legs) while his feet stayed planted behind: the pelvis pitched forward
// over a straight back leg. The harness's fog walk had no such forward contact for him. FIXES: (1) a contact out of his
// arm's vertical span never pulls him (`Fit::plan`), the standoff's limits are scaled to his size; (2) `sanity` below:
// a planned pose with a foot beyond his leg, his pelvis far off his feet or out of his hip height is NOT SHOWN (his own
// pose instead); (3) `WalkThrough`: a borrowed action that WALKS him (the fog, a door's steps) is shown on Donald and Goofy
// as THEIR OWN KH3 WALK (`WalkRunN`, phase by the distance the game moves the body), upright, the staff / shield in hand.

/// THE POSE SANITY of a plan: every planned foot within his leg's reach of its hip, his pelvis over his feet and between a
/// squat's and his standing height. False: the plan would fold or over-stretch him.
pub fn sanity(limbs: &[Option<Vec3>; 4], pelvis: Vec3, t: &Dims, floor: f32, across: Vec3) -> bool {
    if !pelvis.is_finite() || limbs.iter().flatten().any(|l| !l.is_finite()) {
        return false;
    }
    let leg = t.limb[2].max(1.0);
    let hip_c = pelvis - Vec3::Z * (t.pelvis_h - t.hip_h);
    for (j, k) in [2usize, 3].iter().enumerate() {
        let Some(f) = limbs[*k] else { continue };
        let hip = hip_c + across * t.hip_w * if j == 0 { 1.0 } else { -1.0 };
        if (f - hip).length() > 1.12 * leg + 2.0 {
            return false;
        }
    }
    if let (Some(a), Some(b)) = (limbs[2], limbs[3]) {
        let mid = (a + b) * 0.5;
        if Vec3::new(pelvis.x - mid.x, pelvis.y - mid.y, 0.0).length() > 0.6 * leg + 6.0 {
            return false;
        }
    }
    let hips_up = hip_c.z - floor;
    hips_up <= t.hip_h + 0.15 * leg + 2.0 && hips_up >= -2.0
}

/// THE WALK THROUGH (pure): the leader is moved by the game in a borrowed action (the fog's walk, a door's steps): above
/// `ON_SPEED` (cm/s, smoothed) for `ON_S` he walks with his own walk; below `OFF_SPEED` for `OFF_S` the borrowed pose
/// again. `distance` drives the walk's phase; `weight` eases in and out over `BLEND_S`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WalkThrough {
    pub on: bool,
    pub speed: f32,
    pub distance: f32,
    pub weight: f32,
    timer: f32,
    last: Option<Vec3>,
}

impl WalkThrough {
    pub const ON_SPEED: f32 = 25.0;
    pub const OFF_SPEED: f32 = 10.0;
    pub const ON_S: f32 = 0.1;
    pub const OFF_S: f32 = 0.3;
    pub const BLEND_S: f32 = 0.2;
    pub const TAU_S: f32 = 0.15;

    /// One frame: `active` = he is held in a borrowed action; `at` = his feet.
    pub fn frame(&mut self, active: bool, at: Vec3, dt: f32) -> f32 {
        let moved = match self.last {
            Some(l) if dt > 0.0 && (at - l).length() < 200.0 => Vec3::new(at.x - l.x, at.y - l.y, 0.0).length(),
            _ => 0.0,
        };
        self.last = Some(at);
        if dt > 0.0 {
            self.speed += (moved / dt - self.speed) * (1.0 - (-dt / Self::TAU_S).exp());
        }
        if !active {
            self.on = false;
            self.timer = 0.0;
        } else if !self.on {
            self.timer = if self.speed > Self::ON_SPEED { self.timer + dt } else { 0.0 };
            if self.timer >= Self::ON_S {
                self.on = true;
                self.timer = 0.0;
            }
        } else {
            self.timer = if self.speed < Self::OFF_SPEED { self.timer + dt } else { 0.0 };
            if self.timer >= Self::OFF_S {
                self.on = false;
                self.timer = 0.0;
            }
        }
        if self.on {
            self.distance += moved;
        }
        let step = if Self::BLEND_S > 0.0 { dt / Self::BLEND_S } else { 1.0 };
        self.weight = if self.on { (self.weight + step).min(1.0) } else { (self.weight - step).max(0.0) };
        if self.weight <= 0.0 {
            self.distance = 0.0;
        }
        self.weight
    }
}

/// The spine, neck and head bones whose own pose is partly kept (`Profile::style_keep`).
pub const STYLE_BONES: [&str; 7] = ["hara1", "hara2", "mune", "kubi", "kubi1", "kubi2", "atama"];

/// THE PLAN LAID ON A POSE (stateless; his skeleton, component place): the retarget at the plan's pelvis, his own spine
/// / head kept by its share, each planned limb solved to its place (limbs.rs).
#[allow(clippy::too_many_arguments)]
pub fn apply(retarget: &kh_anim::retarget::Retarget, skeleton: &kh_anim::Skeleton, rows: &[(Vec3, Quat)], plan: &Plan, to_world: &kh_anim::Trs, weight: f32, reach: f32, local: &mut [kh_anim::Trs]) {
    if weight <= 0.0 || local.len() != skeleton.len() {
        return;
    }
    let own: Vec<kh_anim::Trs> = local.to_vec();
    retarget.apply(skeleton, rows, plan.pelvis, to_world, weight, local);
    keep_style(skeleton, &own, plan.style_keep.max(plan.spine_keep) * weight, plan.head_only && plan.spine_keep <= 0.0, local);
    keep_head(skeleton, &own, plan.head_keep * weight, local);
    if reach > 0.0 {
        for (k, goal) in plan.limbs.iter().enumerate() {
            if let Some(goal) = goal {
                let to = to_world.rotation.conjugate() * (*goal - to_world.translation);
                // (ANIMS) his knees are solved bending FORWARD (the fog gate's leg kicked out behind was a knee solved the wrong
                // way from a nearly straight leg)
                if k >= 2 {
                    let names = crate::limbs::LIMBS[k];
                    if let (Some(u), Some(l), Some(e)) = (skeleton.index_of(names[0]), skeleton.index_of(names[1]), skeleton.index_of(names[2])) {
                        // (his forward is +Y in his component; a little up: a knee comes up and forward)
                        reach_pole(skeleton, u, l, e, to, Vec3::new(0.0, 1.0, 0.25), weight * reach, local);
                    }
                } else {
                    crate::limbs::reach_sora(skeleton, local, k, to, weight * reach);
                }
            }
        }
    }
}

/// (ANIMS) A TWO-BONE SOLVE WITH A POLE (kh-anim's `reach_arm`, the bend's side given): `upper` / `lower` / `end` bones,
/// the end to `to` (component space) or as near as the limb reaches, the middle joint bent toward `pole` (component
/// direction; a knee: his forward). The end keeps its turn in space. `weight` blends from the pose as it is.
#[allow(clippy::too_many_arguments)]
pub fn reach_pole(skeleton: &kh_anim::Skeleton, upper: usize, lower: usize, end: usize, to: Vec3, pole: Vec3, weight: f32, local: &mut [kh_anim::Trs]) {
    use kh_anim::pose::component_space;
    if weight <= 0.0 || local.len() != skeleton.len() || upper.max(lower).max(end) >= local.len() || !to.is_finite() {
        return;
    }
    let weight = weight.min(1.0);
    let c = component_space(skeleton, local);
    let (s, e, h) = (c[upper].translation, c[lower].translation, c[end].translation);
    let (l1, l2) = ((e - s).length(), (h - e).length());
    if l1 < 1.0e-4 || l2 < 1.0e-4 {
        return;
    }
    let Some(dir) = (to - s).try_normalize() else { return };
    let dist = (to - s).length().clamp((l1 - l2).abs() + 1.0e-3, (l1 + l2) * 0.999);
    let across = |v: Vec3| (v - dir * v.dot(dir)).try_normalize();
    let bend = across(pole).or_else(|| across(e - s)).or_else(|| across(Vec3::Z)).unwrap_or(Vec3::X);
    let cos = ((l1 * l1 + dist * dist - l2 * l2) / (2.0 * l1 * dist)).clamp(-1.0, 1.0);
    let knee = s + dir * (l1 * cos) + bend * (l1 * (1.0 - cos * cos).max(0.0).sqrt());
    let tip = s + dir * dist;
    let first = Quat::from_rotation_arc((e - s) / l1, (knee - s).normalize_or_zero());
    let second = Quat::from_rotation_arc((first * (h - e)).normalize_or_zero(), (tip - knee).normalize_or_zero());
    let end_rotation = c[end].rotation;
    let set = |bone: usize, rotation: Quat, local: &mut [kh_anim::Trs]| {
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
    let c2 = component_space(skeleton, local);
    let mut now = c2[end];
    now.rotation = c[end].rotation.slerp(end_rotation, 1.0);
    if let Some(parent) = skeleton.bones[end].parent {
        local[end] = now.relative_to(&c2[parent]);
    }
}

/// His own spine / neck / head kept by `share` (local rotations slerped back toward `own`).
/// (ANIMS) THE DRAWN POSE'S LIMITS (the user's fog gate: hands up by his head, the head tucked into the shoulders; the
/// coordinator's guards): against HIS OWN pose (`own`, component space) the laid-on one (`now`) must keep
/// - his head on his neck: the neck-to-head line within `HEAD_DEG` of his own;
/// - his chest over his pelvis: the pelvis-to-chest line within `SPINE_DEG` of his own;
/// - his knees bending forward (no knee bent backwards) and not folded shut (`KNEE_MAX_DEG`);
/// - his hands below the top of his head (+ a hand's breadth).
/// `Err` names the first limit broken (the log). Component space (his forward +Y, up +Z).
pub const HEAD_DEG: f32 = 35.0;
pub const SPINE_DEG: f32 = 55.0;
pub const KNEE_MAX_DEG: f32 = 155.0;

pub fn limits(skeleton: &kh_anim::Skeleton, own: &[kh_anim::Trs], now: &[kh_anim::Trs]) -> Result<(), &'static str> {
    let at = |pose: &[kh_anim::Trs], n: &str| skeleton.index_of(n).and_then(|k| pose.get(k)).map(|t| t.translation);
    let neck = |pose: &[kh_anim::Trs]| at(pose, "kubi").or_else(|| at(pose, "kubi1"));
    let angle = |a: Vec3, b: Vec3| a.normalize_or_zero().dot(b.normalize_or_zero()).clamp(-1.0, 1.0).acos().to_degrees();
    // (the chest-to-head line: a duck's neck bone is a few centimetres, its own turn is no measure)
    let _ = neck;
    // (relative to his spine: a lean of the whole body is no tuck; the head bent off the chest's line further than his own is)
    if let (Some(p0), Some(n0), Some(h0), Some(p1), Some(n1), Some(h1)) = (at(own, "center"), at(own, "mune"), at(own, "atama"), at(now, "center"), at(now, "mune"), at(now, "atama")) {
        if (angle(n1 - p1, h1 - n1) - angle(n0 - p0, h0 - n0)).abs() > HEAD_DEG {
            return Err("the head turned off its neck (tucked)");
        }
    }
    if let (Some(p0), Some(c0), Some(p1), Some(c1)) = (at(own, "center"), at(own, "mune"), at(now, "center"), at(now, "mune")) {
        if angle(c0 - p0, c1 - p1) > SPINE_DEG {
            return Err("the spine bent past its limit");
        }
    }
    for side in ["L_", "R_"] {
        let n = |b: &str| format!("{side}{b}");
        let (Some(h), Some(k), Some(a)) = (at(now, &n("momo")), at(now, &n("sune")), at(now, &n("ashi1"))) else { continue };
        let bend = angle(k - h, a - k);
        if bend > KNEE_MAX_DEG {
            return Err("a knee folded shut");
        }
        // forward of the hip-ankle line (his forward +Y), when bent at all
        let line = (a - h).normalize_or_zero();
        let off = (k - h) - line * (k - h).dot(line);
        if bend > 15.0 && off.y < -2.0 {
            return Err("a knee bent backwards");
        }
    }
    if let (Some(top), Some(chest)) = (at(now, "atama"), at(now, "mune")) {
        // (the top of his head: the head bone is its pivot; above it by half the chest-to-head span, or 8 cm)
        let over = (0.6 * (top - chest).length()).max(8.0);
        for hand in ["L_te", "R_te"] {
            if at(now, hand).is_some_and(|p| p.z > top.z + over) {
                return Err("a hand above his head");
            }
        }
    }
    Ok(())
}

/// (ANIMS) His neck and head kept from his own pose by `share` (after `keep_style`: whichever keeps more wins).
pub fn keep_head(skeleton: &kh_anim::Skeleton, own: &[kh_anim::Trs], share: f32, local: &mut [kh_anim::Trs]) {
    if share <= 0.0 || own.len() != local.len() {
        return;
    }
    for name in &STYLE_BONES[3..] {
        if let Some(b) = skeleton.index_of(name) {
            local[b].rotation = local[b].rotation.slerp(own[b].rotation, share.min(1.0)).normalize();
        }
    }
}

pub fn keep_style(skeleton: &kh_anim::Skeleton, own: &[kh_anim::Trs], share: f32, head_only: bool, local: &mut [kh_anim::Trs]) {
    if share <= 0.0 || own.len() != local.len() {
        return;
    }
    for name in STYLE_BONES.iter().skip(if head_only { 3 } else { 0 }) {
        if let Some(b) = skeleton.index_of(name) {
            local[b].rotation = local[b].rotation.slerp(own[b].rotation, share.min(1.0)).normalize();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 1.0 / 60.0;

    /// A Tarnished-sized source standing (floor 0, facing +Y), the limbs' joints at plain places.
    fn tarnished(hands: [Vec3; 2], feet: [Vec3; 2], pelvis: Vec3) -> Vec<(Vec3, Quat)> {
        let mut v = vec![(Vec3::ZERO, Quat::IDENTITY); 52];
        v[rows::PELVIS].0 = pelvis;
        let sh = [pelvis + Vec3::new(17.0, 0.0, 42.0), pelvis + Vec3::new(-17.0, 0.0, 42.0)];
        let hip = [pelvis + Vec3::new(10.0, 0.0, 0.0), pelvis + Vec3::new(-10.0, 0.0, 0.0)];
        for k in 0..2 {
            let [a, b, c] = rows::LIMBS[k];
            v[a].0 = sh[k];
            v[b].0 = (sh[k] + hands[k]) * 0.5 + Vec3::new(0.0, -5.0, 0.0);
            v[c].0 = hands[k];
            let [a, b, c] = rows::LIMBS[2 + k];
            v[a].0 = hip[k];
            v[b].0 = (hip[k] + feet[k]) * 0.5 + Vec3::new(0.0, 5.0, 0.0);
            v[c].0 = feet[k];
        }
        v
    }

    fn s_dims() -> Dims {
        Dims { pelvis_h: 97.0, ankle_h: 11.0, hip_h: 97.0, hip_w: 10.0, head_h: 175.0, shoulder_h: 139.0, shoulder_w: 17.0, limb: [51.4, 51.4, 86.6, 86.6], root: [45.0, 45.0, 10.0, 10.0] }
    }
    fn donald() -> Dims {
        Dims { pelvis_h: 35.0, ankle_h: 6.0, hip_h: 30.0, hip_w: 8.0, head_h: 85.0, shoulder_h: 50.0, shoulder_w: 12.0, limb: [26.0, 26.0, 28.0, 28.0], root: [30.0, 30.0, 8.0, 8.0] }
    }
    fn goofy() -> Dims {
        Dims { pelvis_h: 110.0, ankle_h: 9.0, hip_h: 105.0, hip_w: 10.0, head_h: 190.0, shoulder_h: 150.0, shoulder_w: 18.0, limb: [70.0, 70.0, 100.0, 100.0], root: [55.0, 55.0, 10.0, 10.0] }
    }

    /// A door 45 cm in front of the Tarnished, its hands on it at 120 cm: Donald comes nearer (never nearer than his
    /// depth), Goofy stands back; their arms get the Tarnished's stretch where the plane allows.
    #[test]
    fn the_standoff_brings_short_arms_to_the_door_and_long_ones_back() {
        let src = tarnished([Vec3::new(20.0, 45.0, 120.0), Vec3::new(-20.0, 45.0, 120.0)], [Vec3::new(10.0, 0.0, 11.0), Vec3::new(-10.0, 0.0, 11.0)], Vec3::new(0.0, 0.0, 97.0));
        // (ANIMS 2026-10-23, the fog gate) at 120 cm the hands are far over Donald: he touches the door AT HIS HEIGHT (no hands
        // up to the Tarnished's), stepping no more than his size allows, never through it
        let mut d = Fit::default();
        let high = d.plan(&src, &s_dims(), &donald(), 0.34, &Profile::DONALD, Vec3::ZERO, 0.0, Kind::Surface, DT);
        assert!(high.shift.y <= 0.5 * (26.0 + 28.0) + 0.01 && 45.0 - high.pelvis.y >= Profile::DONALD.depth - 0.01, "{:?}", high);
        for k in 0..2 {
            let hand = high.limbs[k].unwrap();
            assert!((hand.y - 45.0).abs() < 0.01 && hand.z < 85.0, "on the door at his height: {hand}");
        }
        // a door handled at 60 cm: he steps toward it, by no more than his size allows, never through it
        let low = tarnished([Vec3::new(20.0, 45.0, 60.0), Vec3::new(-20.0, 45.0, 60.0)], [Vec3::new(10.0, 0.0, 11.0), Vec3::new(-10.0, 0.0, 11.0)], Vec3::new(0.0, 0.0, 97.0));
        let mut d = Fit::default();
        let pd = d.plan(&low, &s_dims(), &donald(), 0.34, &Profile::DONALD, Vec3::ZERO, 0.0, Kind::Surface, DT);
        assert!(pd.shift.y > 5.0 && pd.shift.y <= 0.5 * (26.0 + 28.0) + 0.01, "Donald steps toward the door: {:?}", pd.shift);
        assert!(45.0 - pd.pelvis.y >= Profile::DONALD.depth - 0.01, "never through it: {:?}", pd.pelvis);
        let mut g = Fit::default();
        let pg = g.plan(&src, &s_dims(), &goofy(), 1.15, &Profile::GOOFY, Vec3::ZERO, 0.0, Kind::Surface, DT);
        assert!(pg.shift.y < 0.0, "Goofy stands back: {:?}", pg.shift);
        // the free kind: no standoff
        let mut f = Fit::default();
        assert_eq!(f.plan(&src, &s_dims(), &donald(), 0.34, &Profile::DONALD, Vec3::ZERO, 0.0, Kind::Free, DT).shift, Vec3::ZERO);
    }

    /// THE WALK THROUGH: the fog's 1.8 m in 3 s (60 cm/s) turns his own walk on within 0.3 s, counts the distance, eases
    /// in; standing (the push into the mist before the walk) it stays off; it ends when the body stops; not in a ladder.
    #[test]
    fn the_walk_through_follows_the_games_move() {
        let mut w = WalkThrough::default();
        // standing 0.5 s: off
        for _ in 0..30 {
            assert_eq!(w.frame(true, Vec3::ZERO, DT), 0.0);
        }
        // walking 60 cm/s
        let mut at = Vec3::ZERO;
        let mut on_at = None;
        for f in 0..180 {
            at.y += 60.0 * DT;
            w.frame(true, at, DT);
            if w.on && on_at.is_none() {
                on_at = Some(f);
            }
        }
        assert!(on_at.unwrap() as f32 * DT < 0.4, "{on_at:?}");
        assert!((w.distance - (180 - on_at.unwrap()) as f32).abs() < 3.0, "{}", w.distance);
        assert_eq!(w.weight, 1.0);
        // stops: off after 0.3 s plus the smoothing, eased out
        let off = (0..120).position(|_| {
            w.frame(true, at, DT);
            !w.on
        });
        assert!(off.unwrap() as f32 * DT < 0.8, "{off:?}");
        // a jump of the place (a warp) is no walking
        let mut j = WalkThrough::default();
        j.frame(true, Vec3::ZERO, DT);
        j.frame(true, Vec3::new(0.0, 5000.0, 0.0), DT);
        assert_eq!(j.speed, 0.0);
        // not allowed (a ladder): never
        let mut l = WalkThrough::default();
        assert!((0..120).all(|f| l.frame(false, Vec3::new(0.0, f as f32, 0.0), DT) == 0.0));
    }

    /// THE POSE SANITY: a foot beyond his leg, his pelvis far off his feet or high over his hips: refused.
    #[test]
    fn the_sanity_refuses_folded_and_overstretched_plans() {
        let t = donald();
        let across = Vec3::X;
        let pelvis = Vec3::new(0.0, 0.0, 6.0 + 24.0 + 5.0);
        let foot = |x: f32, y: f32| Some(Vec3::new(x, y, 6.0));
        let ok = [None, None, foot(8.0, 0.0), foot(-8.0, 0.0)];
        assert!(sanity(&ok, pelvis, &t, 0.0, across));
        assert!(!sanity(&[None, None, foot(8.0, -40.0), foot(-8.0, 0.0)], pelvis, &t, 0.0, across), "a foot kicked out behind");
        assert!(!sanity(&ok, pelvis + Vec3::new(0.0, 30.0, 0.0), &t, 0.0, across), "the pelvis tipped far over the feet");
        assert!(!sanity(&ok, pelvis + Vec3::Z * 30.0, &t, 0.0, across), "lifted off his legs");
        assert!(!sanity(&ok, Vec3::NAN, &t, 0.0, across));
    }

    /// The pelvis's lean is scaled with his legs (about his root), its height about the floor.
    #[test]
    fn the_pelvis_is_placed_at_his_size() {
        let src = tarnished([Vec3::new(20.0, 30.0, 100.0), Vec3::new(-20.0, 30.0, 100.0)], [Vec3::new(10.0, 0.0, 11.0), Vec3::new(-10.0, 0.0, 11.0)], Vec3::new(0.0, 20.0, 90.0));
        let mut d = Fit::default();
        let p = d.plan(&src, &s_dims(), &donald(), 0.34, &Profile::DONALD, Vec3::new(0.0, 0.0, 0.0), 0.0, Kind::Free, DT);
        // his legs are 28 / 86.6 of the Tarnished's: the lean scaled by that, his hips over his ankles as the Tarnished's are, his pelvis
        // bone at his own 5 cm over his hips
        let r = 28.0 / 86.6;
        assert!((p.pelvis.y - 20.0 * r).abs() < 0.01 && (p.pelvis.z - (6.0 + ((90.0 - 11.0) * r).min(Profile::DONALD.hip_cap * 28.0) + 5.0)).abs() < 0.01, "{:?}", p.pelvis);
    }

    /// The fog's walk (1.8 m in 3 s, the Tarnished's strides of 90 cm): Donald's planted feet never slide, and he
    /// steps more often than the Tarnished.
    #[test]
    fn a_walk_inside_an_action_plants_his_feet() {
        let mut d = Fit::default();
        let mut slide = 0.0f32;
        let mut steps = 0;
        let mut last: [Option<(Vec3, bool)>; 2] = [None, None];
        let frames = 180;
        for f in 0..=frames {
            let time = f as f32 * DT;
            let root = Vec3::new(0.0, 60.0 * time, 0.0);
            // the Tarnished's feet: each planted for 0.75 s then swung 90 cm in 0.75 s, alternating
            let foot = |phase: f32, x: f32| {
                let cycle = (time + phase) / 1.5;
                let n = cycle.floor();
                let u = cycle - n;
                let y = if u < 0.5 { n * 90.0 } else { n * 90.0 + 90.0 * ((u - 0.5) / 0.5) };
                let lift = if u < 0.5 { 0.0 } else { 15.0 * (std::f32::consts::PI * (u - 0.5) / 0.5).sin() };
                Vec3::new(x, y - 45.0 + phase * 60.0, 11.0 + lift)
            };
            let src = tarnished([Vec3::new(20.0, root.y + 40.0, 130.0), Vec3::new(-20.0, root.y + 40.0, 130.0)], [foot(0.0, 10.0), foot(0.75, -10.0)], root + Vec3::new(0.0, 0.0, 97.0));
            let plan = d.plan(&src, &s_dims(), &donald(), 0.34, &Profile::DONALD, root, 0.0, Kind::Free, DT);
            for j in 0..2 {
                let at = plan.limbs[2 + j].unwrap();
                let swinging = d.feet[j].swing.is_some();
                if let Some((was, was_swinging)) = last[j] {
                    if !swinging && !was_swinging && d.feet[j].locked.is_some() {
                        slide = slide.max((at - was).length());
                    }
                    if swinging && !was_swinging {
                        steps += 1;
                    }
                }
                last[j] = Some((at, swinging));
            }
        }
        assert!(slide < 0.01, "a planted foot moved {slide} cm in a frame");
        assert!(steps >= 5, "{steps} steps for 1.8 m: he steps at his own stride");
    }

    /// The standoff is smoothed by time, not by frames: 30 and 120 frames a second end in the same place.
    #[test]
    fn the_standoff_is_frame_rate_independent() {
        let src = tarnished([Vec3::new(20.0, 45.0, 120.0), Vec3::new(-20.0, 45.0, 120.0)], [Vec3::new(10.0, 0.0, 11.0), Vec3::new(-10.0, 0.0, 11.0)], Vec3::new(0.0, 0.0, 97.0));
        let far = tarnished([Vec3::new(20.0, 80.0, 120.0), Vec3::new(-20.0, 80.0, 120.0)], [Vec3::new(10.0, 0.0, 11.0), Vec3::new(-10.0, 0.0, 11.0)], Vec3::new(0.0, 0.0, 97.0));
        let run = |hz: f32| {
            let mut d = Fit::default();
            d.plan(&src, &s_dims(), &donald(), 0.34, &Profile::DONALD, Vec3::ZERO, 0.0, Kind::Surface, 1.0 / hz);
            let n = (0.1 * hz) as usize;
            (0..n).map(|_| d.plan(&far, &s_dims(), &donald(), 0.34, &Profile::DONALD, Vec3::ZERO, 0.0, Kind::Surface, 1.0 / hz).shift).last().unwrap()
        };
        let (a, b) = (run(30.0), run(120.0));
        assert!((a - b).length() < 0.5, "{a} {b}");
    }
}
