//! (IMPL_ER83, the user: "KH1 Sora's other hand is still not gripping the keyblade properly", "its the battle stance that
//! KH1 sora holds the keyblade wrong", "his hand is not on his hip here, it looks terrible") HAND CONTACT IK for an outfit
//! whose proportions differ from KH3 Sora's (KH1 only, by the user's choice).
//!
//! The outfit plays KH3 Sora's clips retargeted (outfit.rs: every bone takes his rotation as a change from his reference):
//! on KH1's shorter arms and smaller hands a hand KH3 Sora puts ON something (his Keyblade's grip, his own hip) ends short of
//! it, floats, or sinks into KH1's wider shorts. Each frame, from KH3 Sora's OWN pose of that frame (the host pose the
//! outfit is retargeted from):
//! - THE OFF-HAND ON THE KEYBLADE: his left palm against the Keyblade's grip (the segment of R_buki's X axis the hand can
//!   hold, `GRIP_X`). Contact weight from the palm's distance to it (`W_ON` .. `W_OFF` cm). The outfit's palm goes to the
//!   same place IN THE KEYBLADE'S FRAME (the drawn Keyblade hangs on the outfit's R_buki with his offset: outfit.rs
//!   `weapon_to_world`), with the same palm orientation.
//! - A HAND ON THE BODY (hand on hip, on thigh): his palm against his own body surface (his mesh's vertices without arms,
//!   head and feet, carried by their main bone). Contact weight from the palm's distance to the nearest surface point
//!   (`B_ON` .. `B_OFF` cm). The palm's place is mapped onto the OUTFIT's surface: from the body part's axis (hips:
//!   koshi -> mune, thigh: momo -> sune) along the same direction (in the part's frame), at the same height over the
//!   surface as his palm over his (KH1's shorts are wider: his hand sits on the cloth, not in it, not in the air).
//! - The arm: two-bone IK (upper arm, forearm) to the wrist that puts the palm there, the elbow kept in the plane of the
//!   animated elbow; the hand's rotation the palm's. Blended by the contact weight (0 = the retargeted pose exactly: no
//!   pops, the weight follows his pose continuously).
//! - The fingers: each finger bone of the hand in contact takes his finger's direction in the palm's frame (his curl on the
//!   outfit's finger lengths), by the same weight.
//! Runs in `Retarget::pose_full` after the retarget and the outfit's KineDriver, before its Bonamik and the keychain (which
//! hangs on the drawn hand). kh-er `boc.grip_ik = 0`: off (read every frame).

use std::sync::atomic::{AtomicBool, Ordering};

use glam::{Quat, Vec3};
use kh_anim::{Skeleton, Trs};

static LIVE: AtomicBool = AtomicBool::new(true);

pub fn set_enabled(on: bool) {
    LIVE.store(on, Ordering::Relaxed);
}

pub fn enabled() -> bool {
    LIVE.load(Ordering::Relaxed)
}

static MAIN: AtomicBool = AtomicBool::new(true);

/// (from the host's animator, kh-anim `Animator::own_share`) How much of the pose comes from KH3 Sora's clips (1) rather
/// than the outfit's own (KH1's walk / run / jump: its own hands, left alone).
static KH3_SHARE: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0x3f80_0000);

pub fn set_kh3_share(share: f32) {
    KH3_SHARE.store(share.clamp(0.0, 1.0).to_bits(), Ordering::Relaxed);
}

pub fn kh3_share() -> f32 {
    f32::from_bits(KH3_SHARE.load(Ordering::Relaxed))
}

/// (kh-er `boc.grip_main`) The main hand's fingers take his fist on the Keyblade's grip (his finger directions in the palm's
/// frame) whenever the grip is in it (off: the retargeted fingers).
pub fn set_main_fingers(on: bool) {
    MAIN.store(on, Ordering::Relaxed);
}

pub fn main_fingers() -> bool {
    MAIN.load(Ordering::Relaxed)
}

/// The outfits this runs for (the user: "they seem correct except for KH1").
pub const OUTFITS: [&str; 1] = ["kh1"];

/// The Keyblade's grip along R_buki's X axis (cm): the guard's inside, from the pommel (the keychain's ring at -19.3) to
/// the blade's base (weapon/manifest.json geometry slices; KH3 Sora's main-hand fingers span -5.4 .. +3.6 of it).
pub const GRIP_X: (f32, f32) = (-19.0, 8.0);
/// Off-hand contact with the grip: full weight at or under `W_ON` cm (palm centre to the grip segment), none from `W_OFF`.
pub const W_ON: f32 = 9.0;
pub const W_OFF: f32 = 13.0;
/// A hand on the body: full weight at or under `B_ON` cm (palm centre to his nearest surface point), none from `B_OFF`.
pub const B_ON: f32 = 4.5;
pub const B_OFF: f32 = 8.0;
/// The body target is made while his palm is within this of his surface (a contact ramping out keeps a target).
pub const BODY_REACH: f32 = 20.0;
/// (ER83b) No weight ramp: the contact weight is his pose's, so it fades in and out WITH the clip's own blend (a ramp lagged
/// behind a new clip: the first attack frame got a 22 cm wrist jump where the clip has 14). The drawn frame's TRAVEL LIMIT
/// (`apply_at`) keeps the wrist from ever moving further in a frame than the retargeted wrist does.

const FINGERS: [char; 5] = ['A', 'B', 'C', 'D', 'E'];
/// The grip correction only closes a fist that is already within this of his (cm, last joint to the grip's axis).
const GRIP_FIX_CM: f32 = 1.6;

/// One arm's bones in one skeleton.
#[derive(Clone, Debug)]
struct Arm {
    ude1: usize,
    ude2: usize,
    te: usize,
    /// Finger chains X1, X2, X3 (A = thumb .. E = little finger).
    fingers: [[usize; 3]; 5],
}

/// The bones the solver reads in one skeleton.
#[derive(Clone, Debug)]
struct Bones {
    arms: [Arm; 2],
    buki: usize,
    /// Body parts: torso (koshi -> mune), left thigh, right thigh: (frame bone, axis start, axis end).
    parts: [(usize, usize, usize); 3],
    parent: Vec<Option<usize>>,
    reference: Vec<Trs>,
}

impl Bones {
    fn new(sk: &Skeleton) -> Option<Bones> {
        let i = |n: &str| sk.index_of(n);
        let arm = |s: &str| -> Option<Arm> {
            let f = |x: char| -> Option<[usize; 3]> { Some([i(&format!("{s}_yubi{x}1"))?, i(&format!("{s}_yubi{x}2"))?, i(&format!("{s}_yubi{x}3"))?]) };
            Some(Arm { ude1: i(&format!("{s}_ude1"))?, ude2: i(&format!("{s}_ude2"))?, te: i(&format!("{s}_te"))?, fingers: [f('A')?, f('B')?, f('C')?, f('D')?, f('E')?] })
        };
        let (koshi, mune) = (i("koshi")?, i("mune")?);
        let reference = kh_anim::pose::component_space(sk, &sk.reference_pose());
        Some(Bones {
            arms: [arm("R")?, arm("L")?],
            buki: i("R_buki")?,
            parts: [(koshi, koshi, mune), (i("L_momo")?, i("L_momo")?, i("L_sune")?), (i("R_momo")?, i("R_momo")?, i("R_sune")?)],
            parent: sk.bones.iter().map(|b| b.parent).collect(),
            reference,
        })
    }
}

/// A rigid frame (position, rotation): scale left out.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    pub t: Vec3,
    pub r: Quat,
}

impl Frame {
    fn of(t: &Trs) -> Frame {
        Frame { t: t.translation, r: t.rotation }
    }
    /// This frame expressed in `f`.
    fn rel(&self, f: &Frame) -> Frame {
        let inv = f.r.inverse();
        Frame { t: inv * (self.t - f.t), r: (inv * self.r).normalize() }
    }
    /// A frame given in `f`, in f's parent space.
    fn on(&self, f: &Frame) -> Frame {
        Frame { t: f.t + f.r * self.t, r: (f.r * self.r).normalize() }
    }
}

/// THE PALM'S FRAME of a hand (the same rule on every skeleton): origin between the wrist and the knuckles (0.4 / 0.6),
/// X from the wrist to the knuckles' middle, Y across the knuckles (little finger -> index), Z = X x Y.
fn palm(comps: &[Trs], arm: &Arm) -> Frame {
    let p = |k: usize| comps[k].translation;
    let knuckles = (p(arm.fingers[1][0]) + p(arm.fingers[2][0]) + p(arm.fingers[3][0]) + p(arm.fingers[4][0])) * 0.25;
    let wrist = p(arm.te);
    let x = (knuckles - wrist).normalize_or(Vec3::X);
    let across = p(arm.fingers[1][0]) - p(arm.fingers[4][0]);
    let y = (across - x * across.dot(x)).normalize_or(x.any_orthonormal_vector());
    let z = x.cross(y);
    Frame { t: wrist * 0.4 + knuckles * 0.6, r: Quat::from_mat3(&glam::Mat3::from_cols(x, y, z)).normalize() }
}

/// A body surface: points of a skin in their main bone's frame, with the body part they belong to (0 torso, 1 left leg,
/// 2 right leg).
#[derive(Clone, Debug, Default)]
pub struct Cloud {
    points: Vec<(usize, Vec3, u8)>,
}

impl Cloud {
    /// From a skinned model in its bind pose (`skeleton`'s reference): every vertex whose main joint is under the hips,
    /// a thigh or the spine (not an arm, the head or a foot), one a 1.5 cm cell.
    pub fn from_model(model: &crate::model::Model, sk: &Skeleton) -> Cloud {
        let reference = kh_anim::pose::component_space(sk, &sk.reference_pose());
        let part_of = |mut k: usize| -> Option<u8> {
            loop {
                let name = sk.bones[k].name.as_str();
                match name {
                    "L_ashi1" | "R_ashi1" | "L_kata" | "R_kata" | "kubi" => return None,
                    "L_momo" => return Some(1),
                    "R_momo" => return Some(2),
                    "koshi" | "center" => return Some(0),
                    _ => {}
                }
                k = sk.bones[k].parent?;
            }
        };
        let joints: Vec<Option<usize>> = model.joints.iter().map(|(n, _)| sk.index_of(n)).collect();
        let to_component = crate::math::component_from_pack();
        let mut seen = std::collections::HashSet::new();
        let mut points = Vec::new();
        for v in &model.vertices {
            let (mut best, mut weight) = (0usize, -1.0f32);
            for (j, w) in v.joints.iter().zip(v.weights) {
                if w > weight {
                    best = *j as usize;
                    weight = w;
                }
            }
            let Some(Some(bone)) = joints.get(best) else { continue };
            let Some(part) = part_of(*bone) else { continue };
            let p = to_component.transform_point3(Vec3::from(v.position));
            let cell = (p / 1.5).floor().as_ivec3();
            if !seen.insert((cell, part)) {
                continue;
            }
            let r = &reference[*bone];
            let local = r.rotation.inverse() * (p - r.translation) / r.scale.max(Vec3::splat(1.0e-3));
            points.push((*bone, local, part));
        }
        Cloud { points }
    }

    pub fn len(&self) -> usize {
        self.points.len()
    }

    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }

    /// The points in the pose `comps` (component space) with their part.
    fn posed(&self, comps: &[Trs]) -> Vec<(Vec3, u8)> {
        self.points.iter().filter_map(|(b, l, g)| Some((comps.get(*b)?.transform_point(*l), *g))).collect()
    }
}

/// The nearest point of `pts` to `p`: (distance, part).
fn nearest(pts: &[(Vec3, u8)], p: Vec3) -> Option<(f32, u8)> {
    pts.iter().map(|(q, g)| ((*q - p).length_squared(), *g)).min_by(|a, b| a.0.total_cmp(&b.0)).map(|(d, g)| (d.sqrt(), g))
}

/// The body surface along the ray from `a` in direction `u` (part `g`): the farthest point near the ray (cm from `a`).
fn surface(pts: &[(Vec3, u8)], g: u8, a: Vec3, u: Vec3) -> Option<f32> {
    pts.iter()
        .filter(|(_, pg)| *pg == g)
        .filter_map(|(q, _)| {
            let w = *q - a;
            let along = w.dot(u);
            let perp = (w - u * along).length();
            (along > 0.0 && perp < 1.5 + 0.15 * along).then_some(along)
        })
        .max_by(|a, b| a.total_cmp(b))
}

fn smooth_off(on: f32, off: f32, d: f32) -> f32 {
    let x = ((d - on) / (off - on)).clamp(0.0, 1.0);
    1.0 - x * x * (3.0 - 2.0 * x)
}

/// What the solver found for one hand of one frame (for the proofs too).
#[derive(Clone, Copy, Debug, Default)]
pub struct HandContact {
    /// Weights of the Keyblade contact (off-hand only) and of the body contact.
    pub weapon: f32,
    pub body: f32,
    /// His palm's distance to the grip segment, and to his nearest body surface point (cm).
    pub weapon_d: f32,
    pub body_d: f32,
    /// The body part touched (0 torso / hips, 1 left thigh, 2 right thigh).
    pub part: u8,
    /// The palm frame the outfit's hand is pulled to (outfit component space), at full weight: the Keyblade's (off-hand,
    /// always), the body's (within `BODY_REACH` of his surface), both by their weights.
    pub target: Option<Frame>,
    pub weapon_target: Option<Frame>,
    pub body_target: Option<Frame>,
    /// His palm's height over his surface along the part's ray, and the same for the target over the outfit's (cm).
    pub height_his: f32,
}

impl HandContact {
    pub fn weight(&self) -> f32 {
        self.weapon.max(self.body)
    }
}

/// THE SOLVER for one outfit (made with its skeleton and Sora's, and both skins for the body surfaces).
#[derive(Clone, Debug)]
pub struct HandIk {
    his: Bones,
    its: Bones,
    his_cloud: std::sync::Arc<Cloud>,
    its_cloud: std::sync::Arc<Cloud>,
    /// Per body part: the outfit's reference rotation^-1 * his (a direction in his part's frame -> the outfit's).
    align: [Quat; 3],
    /// Descendants of each arm bone the solver writes (outfit), in the skeleton's order.
    below: Vec<Vec<usize>>,
    /// The main hand's fingers too (his fist on the outfit's fingers whenever he holds the grip).
    pub main_fingers: bool,
    smooth: std::sync::Arc<std::sync::Mutex<Smooth>>,
}

/// (per drawn frame) The weights in use and the body / Keyblade mix of each hand.
#[derive(Clone, Copy, Debug)]
struct Smooth {
    clock: Option<f32>,
    w: [f32; 2],
    body_mix: [f32; 2],
    /// (ER83b) Per hand, of the latest drawn frame: the retargeted wrist (component space), and the wrist and hand
    /// rotation drawn (after the travel limit).
    last_ret: [Option<Vec3>; 2],
    last_drawn: [Option<(Vec3, Quat)>; 2],
}

impl HandIk {
    pub fn new(host: &Skeleton, outfit: &Skeleton, his_cloud: std::sync::Arc<Cloud>, its_cloud: std::sync::Arc<Cloud>) -> Option<HandIk> {
        let his = Bones::new(host)?;
        let its = Bones::new(outfit)?;
        let align = [0, 1, 2].map(|g| (its.reference[its.parts[g].0].rotation.inverse() * his.reference[his.parts[g].0].rotation).normalize());
        let n = its.parent.len();
        let mut below = vec![Vec::new(); n];
        for k in 0..n {
            let mut p = its.parent[k];
            while let Some(a) = p {
                below[a].push(k);
                p = its.parent[a];
            }
        }
        Some(HandIk { his, its, his_cloud, its_cloud, align, below, main_fingers: false, smooth: std::sync::Arc::new(std::sync::Mutex::new(Smooth { clock: None, w: [0.0; 2], body_mix: [0.0; 2], last_ret: [None; 2], last_drawn: [None; 2] })) })
    }

    /// The contacts of both hands (0 right, 1 left) in his component pose `his`, with the outfit's targets in `its`.
    pub fn contacts(&self, his: &[Trs], its: &[Trs]) -> [HandContact; 2] {
        let mut out = [HandContact::default(); 2];
        if his.len() != self.his.parent.len() || its.len() != self.its.parent.len() {
            return out;
        }
        let his_pts = self.his_cloud.posed(his);
        let mut its_pts: Option<Vec<(Vec3, u8)>> = None;
        let grip_his = Frame::of(&his[self.his.buki]);
        let grip_its = Frame::of(&its[self.its.buki]);
        for side in 0..2 {
            let c = &mut out[side];
            let p3 = palm(his, &self.his.arms[side]);
            let mut targets: Vec<(f32, Frame)> = Vec::new();
            // the off-hand on the Keyblade's grip
            if side == 1 {
                let l = p3.rel(&grip_his);
                let on_axis = Vec3::new(l.t.x.clamp(GRIP_X.0, GRIP_X.1), 0.0, 0.0);
                c.weapon_d = (l.t - on_axis).length();
                c.weapon = smooth_off(W_ON, W_OFF, c.weapon_d);
                c.weapon_target = Some(l.on(&grip_its));
                if c.weapon > 0.0 {
                    targets.push((c.weapon, l.on(&grip_its)));
                }
            } else {
                c.weapon_d = f32::INFINITY;
            }
            // a hand on the body
            c.body_d = f32::INFINITY;
            if let Some((d, g)) = nearest(&his_pts, p3.t) {
                c.body_d = d;
                c.part = g;
                let w = smooth_off(B_ON, B_OFF, d);
                if d < BODY_REACH {
                    let pts = its_pts.get_or_insert_with(|| self.its_cloud.posed(its));
                    if let Some(t) = self.body_target(his, its, &his_pts, pts, g, &p3, c) {
                        c.body_target = Some(t);
                        if w > 0.0 {
                            c.body = w;
                            targets.push((w, t));
                        }
                    }
                }
            }
            // both: weighted (continuous when one takes over from the other)
            let total: f32 = targets.iter().map(|(w, _)| w).sum();
            if total > 0.0 {
                let mut t = targets[0].1;
                let mut acc = targets[0].0;
                for (w, f) in &targets[1..] {
                    acc += w;
                    let k = w / acc;
                    t = Frame { t: t.t.lerp(f.t, k), r: t.r.slerp(f.r, k) };
                }
                c.target = Some(t);
            }
        }
        out
    }

    /// His palm `p3` on his body part `g` mapped onto the outfit's surface.
    #[allow(clippy::too_many_arguments)]
    fn body_target(&self, his: &[Trs], its: &[Trs], his_pts: &[(Vec3, u8)], its_pts: &[(Vec3, u8)], g: u8, p3: &Frame, c: &mut HandContact) -> Option<Frame> {
        let (f3, a3, b3) = self.his.parts[g as usize];
        let (f1, a1, b1) = self.its.parts[g as usize];
        let (sa, sb) = (his[a3].translation, his[b3].translation);
        let axis = sb - sa;
        let t = ((p3.t - sa).dot(axis) / axis.length_squared().max(1.0e-6)).clamp(0.0, 1.0);
        let anchor3 = sa + axis * t;
        let rho = (p3.t - anchor3).length();
        let u3 = (p3.t - anchor3).normalize_or_zero();
        if rho < 1.0e-3 {
            return None;
        }
        let r3 = surface(his_pts, g, anchor3, u3)?;
        let height = rho - r3;
        c.height_his = height;
        // the same direction in the outfit's part frame
        let to_its = its[f1].rotation * self.align[g as usize] * his[f3].rotation.inverse();
        let u1 = (to_its * u3).normalize();
        let anchor1 = its[a1].translation.lerp(its[b1].translation, t);
        let r1 = surface(its_pts, g, anchor1, u1)?;
        Some(Frame { t: anchor1 + u1 * (r1 + height), r: (to_its * p3.r).normalize() })
    }

    /// THE SOLVE on the outfit's pose (`locals`, `comps`: after the retarget and its KineDriver) for his pose `his`.
    pub fn apply(&self, his: &[Trs], locals: &mut [Trs], comps: &mut [Trs]) -> [HandContact; 2] {
        self.apply_at(his, locals, comps, None)
    }

    /// `apply` for the drawn frame of `clock` (s): the first call of a new clock solves the frame under the travel limit (its
    /// wrists move at most as far as the retargeted ones since the frame before); a call again for that clock, or without a
    /// clock after one, gives the same; before any clock: the contacts' own solve, no limit.
    pub fn apply_at(&self, his: &[Trs], locals: &mut [Trs], comps: &mut [Trs], clock: Option<f32>) -> [HandContact; 2] {
        let mut contacts = [HandContact::default(); 2];
        if his.len() != self.his.parent.len() || comps.len() != self.its.parent.len() || locals.len() != comps.len() {
            return contacts;
        }
        let mut s = self.smooth.lock().unwrap_or_else(|e| e.into_inner());
        // a new drawn frame steps the weights and the travel limit; a call again for the same frame (or without a clock
        // after one) draws what that frame drew; without any clock yet: the contacts' own weights, no limit
        let fresh = clock.is_some() && s.clock != clock;
        let stepped = s.clock.is_some();
        // the main hand first: the Keyblade (and so the off-hand's target) follows it
        for side in 0..2 {
            let found = self.contacts(his, comps)[side];
            contacts[side] = found;
            let share = kh3_share();
            let raw = found.weight() * share;
            if raw > 0.0 && (fresh || !stepped) {
                s.body_mix[side] = found.body / (found.body + found.weapon);
            }
            if fresh {
                s.w[side] = raw;
            }
            let (w, body_mix) = if stepped || fresh { (s.w[side], s.body_mix[side]) } else { (raw, s.body_mix[side]) };
            let target = match (found.weapon_target, found.body_target) {
                (Some(a), Some(b)) => Some(Frame { t: a.t.lerp(b.t, body_mix), r: a.r.slerp(b.r, body_mix) }),
                // only the kind the weight is made of (a hand leaving his hip is not pulled to the Keyblade)
                (Some(a), None) => (body_mix < 0.5).then_some(a),
                (None, b) => b.filter(|_| body_mix >= 0.5 || side == 0),
            };
            let te = self.its.arms[side].te;
            let (ret, ret_r) = (comps[te].translation, comps[te].rotation);
            let (want, want_r) = match target {
                Some(t) if w > 1.0e-4 => self.wrist_for(side, t, w, comps),
                _ => (ret, ret_r),
            };
            // (ER83b, main: "the IK must never make a jump worse") THE TRAVEL LIMIT: the drawn wrist moves in one frame at
            // most as far as the retargeted wrist does; a contact that ends or starts with a clip change (an attack, a
            // guard) fades with the clip's own motion instead of adding a jump to it
            let (wrist, rot) = if fresh {
                let drawn = match (s.last_ret[side], s.last_drawn[side]) {
                    (Some(lr), Some((lw, lq))) => {
                        let budget = (ret - lr).length();
                        let d = want - lw;
                        if d.length() > budget + 1.0e-5 {
                            let g = budget / d.length();
                            (lw + d * g, lq.slerp(want_r, g).normalize())
                        } else {
                            (want, want_r)
                        }
                    }
                    _ => (want, want_r),
                };
                s.last_ret[side] = Some(ret);
                drawn
            } else if stepped {
                s.last_drawn[side].unwrap_or((want, want_r))
            } else {
                (want, want_r)
            };
            if (wrist - ret).length() > 1.0e-4 || rot.dot(ret_r).abs() < 0.999_999 {
                self.reach_wrist(side, wrist, rot, locals, comps);
            }
            if fresh {
                // what was drawn (the arm may not reach the wrist asked for)
                s.last_drawn[side] = Some((comps[te].translation, comps[te].rotation));
            }
            let fingers = if side == 0 && (self.main_fingers || main_fingers()) { share } else { w };
            if fingers > 1.0e-4 {
                self.curl(side, his, fingers, locals, comps);
            }
        }
        if fresh {
            s.clock = clock;
        }
        contacts
    }

    /// Sets bone `k`'s component transform (rotation, position) and carries its descendants.
    fn put(&self, k: usize, rotation: Quat, translation: Vec3, locals: &mut [Trs], comps: &mut [Trs]) {
        comps[k] = Trs { translation, rotation: rotation.normalize(), scale: comps[k].scale };
        locals[k] = match self.its.parent[k] {
            Some(p) => comps[k].relative_to(&comps[p]),
            None => comps[k],
        };
        for &d in &self.below[k] {
            if let Some(p) = self.its.parent[d] {
                comps[d] = locals[d].then(&comps[p]);
            }
        }
    }

    /// The wrist and hand rotation that put the palm of `side` on `target` by weight `w`.
    fn wrist_for(&self, side: usize, target: Frame, w: f32, comps: &[Trs]) -> (Vec3, Quat) {
        let arm = &self.its.arms[side];
        let now = palm(comps, arm);
        let palm_in_te = now.rel(&Frame::of(&comps[arm.te]));
        let want = Frame { t: now.t.lerp(target.t, w), r: now.r.slerp(target.r, w) };
        // the wrist that puts the palm there
        let te_r = (want.r * palm_in_te.r.inverse()).normalize();
        (want.t - te_r * palm_in_te.t, te_r)
    }

    /// The arm of `side` with its wrist at `wrist` and the hand turned `te_r` (two-bone IK, the animated elbow's plane).
    fn reach_wrist(&self, side: usize, wrist: Vec3, te_r: Quat, locals: &mut [Trs], comps: &mut [Trs]) {
        let arm = &self.its.arms[side];
        let (s, e, h) = (comps[arm.ude1].translation, comps[arm.ude2].translation, comps[arm.te].translation);
        let (a, b) = ((e - s).length(), (h - e).length());
        let to = wrist - s;
        let c = to.length().clamp((a - b).abs() + 0.01, a + b - 0.01);
        if c < 1.0e-3 || a < 1.0e-3 || b < 1.0e-3 {
            return;
        }
        let dir = to.normalize_or(Vec3::X);
        let goal = s + dir * c;
        let bend = (e - s) - dir * (e - s).dot(dir);
        let pole = if bend.length() > 1.0e-3 { bend.normalize() } else { dir.any_orthonormal_vector() };
        let cos_a = ((a * a + c * c - b * b) / (2.0 * a * c)).clamp(-1.0, 1.0);
        let elbow = s + dir * (a * cos_a) + pole * (a * (1.0 - cos_a * cos_a).max(0.0).sqrt());
        let r1 = Quat::from_rotation_arc((e - s).normalize(), (elbow - s).normalize());
        let ude1 = comps[arm.ude1];
        self.put(arm.ude1, r1 * ude1.rotation, ude1.translation, locals, comps);
        let (e2, h2) = (comps[arm.ude2].translation, comps[arm.te].translation);
        let r2 = Quat::from_rotation_arc((h2 - e2).normalize(), (goal - e2).normalize());
        let ude2 = comps[arm.ude2];
        self.put(arm.ude2, r2 * ude2.rotation, ude2.translation, locals, comps);
        let te = comps[arm.te].translation;
        self.put(arm.te, te_r, te, locals, comps);
    }

    /// His finger directions (in his palm's frame) on the outfit's fingers of `side`, by weight `w`.
    fn curl(&self, side: usize, his: &[Trs], w: f32, locals: &mut [Trs], comps: &mut [Trs]) {
        let (h, o) = (&self.his.arms[side], &self.its.arms[side]);
        let p3 = palm(his, h);
        for f in 0..FINGERS.len() {
            for j in 0..2 {
                let (hb, hc) = (h.fingers[f][j], h.fingers[f][j + 1]);
                let (ob, oc) = (o.fingers[f][j], o.fingers[f][j + 1]);
                let want_in_palm = p3.r.inverse() * (his[hc].translation - his[hb].translation).normalize_or_zero();
                let p1 = palm(comps, o);
                let want = (p1.r * want_in_palm).normalize_or_zero();
                let cur = (comps[oc].translation - comps[ob].translation).normalize_or_zero();
                if want == Vec3::ZERO || cur == Vec3::ZERO {
                    continue;
                }
                let swing = Quat::IDENTITY.slerp(Quat::from_rotation_arc(cur, want), w);
                let b = comps[ob];
                self.put(ob, swing * b.rotation, b.translation, locals, comps);
            }
        }
        if side == 0 {
            self.close_grip(his, w, locals, comps);
        }
    }

    /// THE GRIP CORRECTION for the outfit's hand size (main hand): where his finger (index .. little) closes on the
    /// Keyblade's grip, the outfit's finger is curled further (both joints by the same angle, -10 .. +15 degrees) until its
    /// last joint is as far from the grip's axis as his (the same Keyblade: the same grip to close on).
    fn close_grip(&self, his: &[Trs], w: f32, locals: &mut [Trs], comps: &mut [Trs]) {
        let (h, o) = (&self.his.arms[0], &self.its.arms[0]);
        let (gh, go) = (Frame::of(&his[self.his.buki]), Frame::of(&comps[self.its.buki]));
        let radial = |p: Vec3, g: &Frame| {
            let l = g.r.inverse() * (p - g.t);
            (l.x, (l.y * l.y + l.z * l.z).sqrt())
        };
        for f in 1..FINGERS.len() {
            let (x3, r3) = radial(his[h.fingers[f][2]].translation, &gh);
            if !(GRIP_X.0..=GRIP_X.1).contains(&x3) || r3 > 6.0 {
                continue;
            }
            let [a, b, c] = o.fingers[f];
            let (p1, p2, p3) = (comps[a].translation, comps[b].translation, comps[c].translation);
            let n = (p2 - p1).cross(p3 - p2);
            if n.length() < 1.0e-4 {
                continue;
            }
            let n = n.normalize();
            let tip = |deg: f32| {
                let q = Quat::from_axis_angle(n, deg.to_radians());
                let q2 = p1 + q * (p2 - p1);
                radial(q2 + q * (q * (p3 - p2)), &go).1
            };
            // a fine correction of a fist only (the outfit's own clips, played through his skeleton, are left alone)
            if (tip(0.0) - r3).abs() > GRIP_FIX_CM {
                continue;
            }
            let best = (-4..=6).map(|k| k as f32 * 2.5).min_by(|x, y| (tip(*x) - r3).abs().total_cmp(&(tip(*y) - r3).abs())).unwrap_or(0.0) * w;
            if best.abs() < 1.0e-3 {
                continue;
            }
            let q = Quat::from_axis_angle(n, best.to_radians());
            let ta = comps[a];
            self.put(a, q * ta.rotation, ta.translation, locals, comps);
            let tb = comps[b];
            self.put(b, q * tb.rotation, tb.translation, locals, comps);
        }
    }

    /// FOR THE PROOFS: per hand, the outfit's palm's distance to the target it should hold (cm, at full contact the
    /// target itself), the palm's distance to the Keyblade's grip segment (off-hand) and over the outfit's body surface.
    pub fn measure(&self, his: &[Trs], its: &[Trs]) -> [(HandContact, f32, f32); 2] {
        let c = self.contacts(his, its);
        let grip = Frame::of(&its[self.its.buki]);
        [0, 1].map(|side| {
            let p1 = palm(its, &self.its.arms[side]);
            let off = c[side].target.map_or(f32::NAN, |t| (t.t - p1.t).length());
            let l = p1.rel(&grip);
            let grip_d = (l.t - Vec3::new(l.t.x.clamp(GRIP_X.0, GRIP_X.1), 0.0, 0.0)).length();
            (c[side], off, grip_d)
        })
    }

    /// FOR THE PROOFS: the palm frames (his, the outfit's) of `side` in their own R_buki frame.
    pub fn palms_in_grip(&self, his: &[Trs], its: &[Trs], side: usize) -> (Frame, Frame) {
        (palm(his, &self.his.arms[side]).rel(&Frame::of(&his[self.his.buki])), palm(its, &self.its.arms[side]).rel(&Frame::of(&its[self.its.buki])))
    }

    /// FOR THE PROOFS: the outfit palm's height over the outfit's body surface near it (nearest surface point, cm).
    pub fn body_gap(&self, its: &[Trs], side: usize) -> f32 {
        let pts = self.its_cloud.posed(its);
        nearest(&pts, palm(its, &self.its.arms[side]).t).map_or(f32::NAN, |(d, _)| d)
    }

    /// FOR THE PROOFS: his palm's distance to his nearest surface point.
    pub fn body_gap_his(&self, his: &[Trs], side: usize) -> f32 {
        let pts = self.his_cloud.posed(his);
        nearest(&pts, palm(his, &self.his.arms[side]).t).map_or(f32::NAN, |(d, _)| d)
    }
}

/// The clouds of the skins, made once per file.
static CLOUDS: std::sync::Mutex<Vec<(std::path::PathBuf, std::sync::Arc<Cloud>)>> = std::sync::Mutex::new(Vec::new());

/// The body surface of a skin file (cached).
pub fn cloud_of(file: &std::path::Path, sk: &Skeleton) -> Result<std::sync::Arc<Cloud>, String> {
    if let Some(c) = CLOUDS.lock().ok().and_then(|g| g.iter().find(|(f, _)| f == file).map(|(_, c)| c.clone())) {
        return Ok(c);
    }
    let model = crate::model::Model::load(file)?;
    let cloud = std::sync::Arc::new(Cloud::from_model(&model, sk));
    if let Ok(mut g) = CLOUDS.lock() {
        g.push((file.to_path_buf(), cloud.clone()));
    }
    Ok(cloud)
}

#[cfg(test)]
mod tests {
    use super::*;
    use kh_anim::pose::component_space;

    fn setup() -> Option<(kh_anim::Animator, crate::outfit::Retarget)> {
        let pack = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_pack");
        if !pack.join("outfits/kh1/manifest.json").is_file() {
            eprintln!("SKIPPED: no pack with the kh1 outfit");
            return None;
        }
        let a = kh_anim::Animator::new(kh_anim::Pack::load(&pack).ok()?, kh_anim::Settings::default()).ok()?;
        let started = std::time::Instant::now();
        let r = crate::outfit::retarget_for(&pack, "kh1", a.skeleton()).ok()??;
        eprintln!("kh1 retarget with its hand IK made in {:.0} ms", started.elapsed().as_secs_f32() * 1000.0);
        Some((a, r))
    }

    /// The battle stance (1015): KH3 Sora's left hand lies over his right fist on the grip; KH1's goes to the same place in
    /// the Keyblade's frame (it was 3-4 cm off, into the fist), the hip hand of the idle break (1004) onto KH1's shorts.
    #[test]
    fn kh1s_hands_go_where_kh3_soras_touch() {
        let Some((a, r)) = setup() else { return };
        let ik = r.hand_ik.clone().expect("kh1 has the hand IK");
        let mut plain = r.clone();
        plain.hand_ik = None;
        let sk = a.skeleton().clone();
        for (clip, t, side, kind) in [("1015", 1.0, 1, 0), ("1018", 0.5, 1, 0), ("1004", 3.0, 1, 1)] {
            let local = a.sample(clip, t);
            let his = component_space(&sk, &local);
            let (_, before) = plain.pose_full(&local, &his);
            let (_, after) = r.pose_full(&local, &his);
            let (b, a2) = (ik.measure(&his, &before)[side], ik.measure(&his, &after)[side]);
            let w = if kind == 0 { b.0.weapon } else { b.0.body };
            assert!(w > 0.99, "{clip}: contact {w}");
            assert!(b.1 > 2.0, "{clip}: the retargeted hand was {:.1} cm off", b.1);
            assert!(a2.1 < 0.5, "{clip}: the hand is {:.2} cm from where his is", a2.1);
            if kind == 1 {
                // over KH1's surface as high as his palm over his (within 1.5 cm)
                let (his_gap, its_gap) = (ik.body_gap_his(&his, side), ik.body_gap(&after, side));
                assert!((his_gap - its_gap).abs() < 1.5, "{clip}: his {his_gap:.1} KH1 {its_gap:.1}");
            }
        }
        // no contact: the retargeted pose exactly (the field run's hands)
        let local = a.sample("0003", 0.3);
        let his = component_space(&sk, &local);
        let c = ik.contacts(&his, &plain.pose_full(&local, &his).1);
        assert!(c[0].weight() == 0.0 && c[1].weight() == 0.0, "{c:?}");
    }

    /// (ER83b) THE TRAVEL LIMIT: from the field run into the battle stance over a 12-frame blend (as a clip change), the
    /// drawn wrists never move further in a frame than the retargeted ones, and the left hand is on its target at the end;
    /// back to the run in ONE frame (a snap), the IK adds nothing to the snap.
    #[test]
    fn the_ik_never_moves_a_wrist_further_than_the_clip() {
        let Some((a, r)) = setup() else { return };
        let ik = r.hand_ik.clone().unwrap();
        let mut plain = r.clone();
        plain.hand_ik = None;
        let sk = a.skeleton().clone();
        let run = a.sample("0003", 0.3);
        let stance = |k: usize| a.sample("1015", 1.0 + k as f32 / 60.0);
        let blend = |k: usize| -> Vec<Trs> { let f = k as f32 / 12.0; run.iter().zip(&stance(k)).map(|(x, y)| Trs { translation: x.translation.lerp(y.translation, f), rotation: x.rotation.slerp(y.rotation, f).normalize(), scale: x.scale.lerp(y.scale, f) }).collect() };
        // 12 frames of blend into the stance (breathing), 3 s of stance, then a snap back to the run
        let mut frames: Vec<Vec<Trs>> = (0..=12).map(blend).collect();
        frames.extend((13..=192).map(stance));
        frames.push(run.clone());
        let check = frames.len() - 2;
        let te = [ik.its.arms[0].te, ik.its.arms[1].te];
        let (mut last_ik, mut last_ret): (Option<[Vec3; 2]>, Option<[Vec3; 2]>) = (None, None);
        let mut end_gap = 0.0;
        for (k, local) in frames.iter().enumerate() {
            let his = component_space(&sk, local);
            let (mut l, mut c) = plain.pose_full(local, &his);
            let ret = [c[te[0]].translation, c[te[1]].translation];
            let found = ik.apply_at(&his, &mut l, &mut c, Some(k as f32 / 60.0));
            let drawn = [c[te[0]].translation, c[te[1]].translation];
            if let (Some(li), Some(lr)) = (last_ik, last_ret) {
                for h in 0..2 {
                    let (moved, clip) = ((drawn[h] - li[h]).length(), (ret[h] - lr[h]).length());
                    assert!(moved <= clip + 1.0e-3, "frame {k} hand {h}: the wrist moved {moved:.3} cm, the clip {clip:.3}");
                }
            }
            if k % 30 == 0 || k == check {
                let gap = (palm(&c, &ik.its.arms[1]).t - found[1].weapon_target.unwrap().t).length();
                eprintln!("frame {k}: left palm {gap:.2} cm from its target");
                end_gap = gap;
            }
            (last_ik, last_ret) = (Some(drawn), Some(ret));
        }
        assert!(end_gap < 0.5, "the left palm ends {end_gap:.2} cm from its target");
    }
}
