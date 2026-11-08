//! (CUTSCENE-ANIM 2026-11-06) THE LEADER IN A REAL-TIME CUTSCENE, AT HIS OWN PROPORTIONS, ON THE SCENE'S FLOOR.
//!
//! THE USER: "Fix the party leader's animations in cutscenes, also making sure KH1 Sora works properly as well. They need to
//! animate perfectly regardless of who is party leader."
//!
//! THE ROUTE (IMPL_ER16, figure.rs): a real-time cutscene animates its own figure of the player (`CSViewerModelIns` c0000,
//! the remo's c0000 track); it is held hidden and its 52 `ROWS` bones are read every frame, retargeted onto the leader's
//! skeleton (kh-anim `Retarget`: every bone points where the figure's points) and laid over his animator's pose. Sora is
//! drawn there in any outfit (the renderer poses the outfit from his pose), a friend leader (Donald, Goofy) by the party's
//! drawing (party/leader.rs `PoseOver`, figure.rs `posed_party`).
//!
//! WHAT WAS WRONG (measured offline: cutfit_sheet.rs, build/sora_tmp/weapons/CUTSCENE-ANIM):
//! - THE HEIGHT. The pelvis was put at the figure's pelvis height scaled about "the floor" by the pelvis BONE's height
//!   ratio, the floor guessed as the figure's lowest bone less 4 cm. The pelvis bone ratio is not the leg ratio (Sora's
//!   `center` sits 12 cm over his hips, Donald's legs are 0.22 of the Tarnished's for a pelvis ratio of 0.34), and a bone
//!   is not a sole: standing, kneeling, sitting or lying, the leader floated or sank by centimetres (Donald by a hand).
//! - THE CONTACTS. Nothing brought a hand or a foot to what the figure's touches: a hand pushing up off the floor hung over
//!   it, a planted foot hovered when the other carried the weight, the item the figure takes was held in the air.
//! - THE OUTFIT. The height was Sora's skeleton's; the outfit drawn (KH1: its hips 0.87 of his, its own leg proportions)
//!   is posed from his translations scaled, so its soles were not where his were.
//!
//! WHAT IT IS (`fit`, pure; once a frame where the pose is made, every leader the same way):
//! - THE FLOOR: the figure's lowest part less that part's own flesh (a sole, a knee, a hand, the seat, the chest, the
//!   head: `Body::flesh`), so whatever the figure rests on is the floor, standing or lying.
//! - THE HEIGHT: the leader's pose made with his pelvis at the figure's, measured the same way ON THE SKELETON THAT IS DRAWN
//!   (the worn outfit's for Sora), and lifted / lowered by the difference: a knee, the seat, the chest or the head the figure
//!   rests on carries him the same way (kneeling, sitting, lying, on all fours); standing, his lowest part rests where the
//!   figure's does. A hand or a sole of his that would still be under the floor (Goofy's long arms, Donald's short legs) is
//!   brought up onto it by its limb. A hand on the floor out of his reach while he stands (an item picked up) crouches him,
//!   his feet kept on the floor, never past his knees' fold.
//! - THE CONTACTS: a part of the figure's on the floor (a foot, a hand) is put on the floor for him (a two-bone solve, his
//!   end keeps its turn); a hand that holds something away from the figure's body (an item given, a rein, a door) is brought
//!   to it when his arm is within reach and not much shorter than the Tarnished's (Sora in every outfit, Goofy); a short arm
//!   (Donald) keeps the figure's arm direction at his own length (reaching would raise it over his head: ANIMS' fog gate).
//! - Stateless and continuous (a minimum and smooth weights of the figure's pose): frame-rate independent, no pop, a camera
//!   cut's jump is the figure's own.

use glam::{Quat, Vec3};

use crate::propfit::Dims;

/// The parts a body may rest on: `ROWS` row (figure.rs) and the KH bone of each.
pub const CONTACTS: [(usize, &str); 11] = [(0, "center"), (3, "mune"), (5, "atama"), (9, "L_te"), (13, "R_te"), (15, "L_sune"), (19, "R_sune"), (16, "L_ashi1"), (20, "R_ashi1"), (17, "L_ashi2"), (21, "R_ashi2")];
const PELVIS: usize = 0;
const CHEST: usize = 1;
const HEAD: usize = 2;
const HANDS: [usize; 2] = [3, 4];
const KNEES: [usize; 2] = [5, 6];
const ANKLES: [usize; 2] = [7, 8];
const TOES: [usize; 2] = [9, 10];
/// The parts that carry the body when they rest on the floor (the seat, the chest, the head, the knees): not limb ends.
const TRUNK: [usize; 5] = [PELVIS, CHEST, HEAD, KNEES[0], KNEES[1]];
/// The upper arms' and thighs' roots (row, KH bone), left then right.
pub const SHOULDERS: [(usize, &str); 2] = [(7, "L_ude1"), (11, "R_ude1")];
pub const HIPS: [(usize, &str); 2] = [(14, "L_momo"), (18, "R_momo")];

/// A part this near its floor (cm) is ON it (full weight); this far, off it (none); smooth between.
pub const ON_FLOOR: f32 = 2.0;
pub const OFF_FLOOR: f32 = 7.0;
/// A hand is brought to the figure's hand only for arms at least this share of the Tarnished's.
pub const PROP_ARM_RATIO: f32 = 0.65;
/// A hand of the figure's within this of its own body (cm) touches itself (hands on knees, on the chest): no world contact.
pub const SELF_TOUCH: f32 = 16.0;
/// A foot is put on the floor only when its sole is within this share of his leg of it (no leg thrown about).
pub const FOOT_FIX_SHARE: f32 = 0.5;
/// A hand or a sole of his this far under the floor (cm) is put on it whatever the figure's does (smooth from 0).
pub const UNDER_FLOOR: f32 = 2.0;
/// A crouch keeps his hips at least this share of his leg over his ankles (the knees' fold).
pub const CROUCH_MIN_HIPS: f32 = 0.22;
/// The flesh under an ankle lying on its side (a shin along the floor), as a share of the standing sole's.
pub const ANKLE_SIDE: f32 = 0.45;
/// The Tarnished's body depth (cm, front to back at the chest; the ER human, by eye) for its flesh.
pub const TARNISHED_DEPTH: f32 = 14.0;

/// The places of a body's parts (world, KH3 cm).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Points {
    /// In `CONTACTS` order.
    pub contact: [Vec3; 11],
    pub shoulder: [Vec3; 2],
    pub hip: [Vec3; 2],
}

impl Points {
    /// The figure's (every `ROWS` row's bone).
    pub fn of_rows(src: &[(Vec3, Quat)]) -> Option<Points> {
        let at = |k: usize| src.get(k).map(|r| r.0);
        Some(Points { contact: try_map(CONTACTS.map(|(k, _)| at(k)))?, shoulder: try_map(SHOULDERS.map(|(k, _)| at(k)))?, hip: try_map(HIPS.map(|(k, _)| at(k)))? })
    }

    /// A KH skeleton's (`at`: a bone's place by name).
    pub fn of_names(at: &dyn Fn(&str) -> Option<Vec3>) -> Option<Points> {
        Some(Points { contact: try_map(CONTACTS.map(|(_, n)| at(n)))?, shoulder: try_map(SHOULDERS.map(|(_, n)| at(n)))?, hip: try_map(HIPS.map(|(_, n)| at(n)))? })
    }

    pub fn moved(&self, by: Vec3) -> Points {
        Points { contact: self.contact.map(|p| p + by), shoulder: self.shoulder.map(|p| p + by), hip: self.hip.map(|p| p + by) }
    }

    /// How upright the trunk is (0 lying / upside down, 1 upright).
    fn upright(&self) -> f32 {
        (self.contact[CHEST] - self.contact[PELVIS]).normalize_or_zero().z.max(0.0)
    }
}

fn try_map<const N: usize>(a: [Option<Vec3>; N]) -> Option<[Vec3; N]> {
    a.iter().all(Option::is_some).then(|| a.map(|v| v.unwrap_or(Vec3::ZERO)))
}

/// A body's measures for the fit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Body {
    pub dims: Dims,
    /// Front to back at the chest (cm).
    pub depth: f32,
}

impl Body {
    /// A KH skeleton's measures from its reference pose (`at`: a bone's place by name, component space; Sora, Donald, Goofy
    /// and every outfit share the names; Goofy's neck is not needed).
    pub fn of_named(at: &dyn Fn(&str) -> Option<Vec3>, depth: f32) -> Option<Body> {
        let names = crate::limbs::LIMBS;
        let mut limbs = [[Vec3::ZERO; 3]; 4];
        for k in 0..4 {
            for j in 0..3 {
                limbs[k][j] = at(names[k][j])?;
            }
        }
        let floor = at("L_ashi2")?.z.min(at("R_ashi2")?.z).min(limbs[2][2].z);
        let mut dims = Dims::from_points(at("center")?, limbs, floor)?;
        if let (Some(head), Some(chest)) = (at("atama"), at("mune")) {
            dims.head_h = crate::propfit::head_top(head, chest) - floor;
        }
        Some(Body { dims, depth })
    }

    /// The flesh under a part resting on the floor (cm from the bone to the floor) in the pose `p` (the trunk upright or lying,
    /// a shin upright or lying along the floor).
    pub fn flesh(&self, part: usize, p: &Points) -> f32 {
        let d = &self.dims;
        let upright = p.upright();
        match part {
            // (the seat: the buttocks under the hips, and a pelvis bone over its hips (Sora's `center`) when sitting up)
            PELVIS => 0.5 * self.depth + (d.pelvis_h - d.hip_h).max(0.0) * upright,
            CHEST => 0.6 * self.depth,
            HEAD => 0.3 * (d.head_h - d.shoulder_h).max(0.0),
            3 | 4 => 0.06 * 0.5 * (d.limb[0] + d.limb[1]),
            5 | 6 => 0.06 * 0.5 * (d.limb[2] + d.limb[3]),
            // (a sole under the ankle while the shin stands; a shin lying along the floor (kneeling, lying) rests on the ankle's
            // side, a little under it)
            7 | 8 => {
                let j = part - 7;
                let shin_up = (p.contact[KNEES[j]] - p.contact[ANKLES[j]]).normalize_or_zero().z.max(0.0);
                d.ankle_h * (ANKLE_SIDE + (1.0 - ANKLE_SIDE) * shin_up)
            }
            // (the floor is at the toes in both reference poses)
            _ => 0.0,
        }
    }

    /// The floor the body rests on: its lowest part less that part's flesh. Returns (the floor, the part).
    pub fn rest(&self, p: &Points) -> (f32, usize) {
        (0..CONTACTS.len()).map(|k| (p.contact[k].z - self.flesh(k, p), k)).fold((f32::MAX, 0), |a, b| if b.0 < a.0 { b } else { a })
    }

    fn arm(&self, k: usize) -> f32 {
        self.dims.limb[k]
    }

    fn leg(&self, j: usize) -> f32 {
        self.dims.limb[2 + j]
    }
}

/// A limb's end to a place (world), by a weight.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Goal {
    pub to: Vec3,
    pub weight: f32,
}

/// The fit of one frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fit {
    /// The floor the figure rests on (world z).
    pub floor: f32,
    /// What his whole body is moved up (down if negative) from the pose made with his pelvis at the figure's.
    pub lift: f32,
    /// The part of the figure's that rests lowest, and his.
    pub rests_on: (usize, usize),
    /// Each limb's goal (limbs.rs order: left hand, right hand, left foot, right foot); None = as the retarget turned it.
    pub limbs: [Option<Goal>; 4],
}

fn smooth(off: f32, on: f32, x: f32) -> f32 {
    let t = ((x - off) / (on - off)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn segment_distance(p: Vec3, a: Vec3, b: Vec3) -> f32 {
    let ab = b - a;
    let t = if ab.length_squared() > 1.0e-6 { ((p - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0) } else { 0.0 };
    (p - (a + ab * t)).length()
}

/// A hand of the figure's on its own body (the trunk, the head, a thigh, a shin): no world contact to keep.
fn touches_itself(p: &Points, hand: usize) -> bool {
    let h = p.contact[HANDS[hand]];
    let segments = [(p.contact[PELVIS], p.contact[CHEST]), (p.contact[CHEST], p.contact[HEAD]), (p.hip[0], p.contact[KNEES[0]]), (p.hip[1], p.contact[KNEES[1]]), (p.contact[KNEES[0]], p.contact[ANKLES[0]]), (p.contact[KNEES[1]], p.contact[ANKLES[1]])];
    segments.iter().any(|(a, b)| segment_distance(h, *a, *b) < SELF_TOUCH)
}

/// THE FIT OF A FRAME. `src` / `s`: the figure's parts and measures; `tgt` / `t`: his, as his pose is with his pelvis at the
/// figure's pelvis (on the skeleton that is drawn).
pub fn fit(src: &Points, s: &Body, tgt: &Points, t: &Body) -> Fit {
    let (floor, src_part) = s.rest(src);
    let on = |k: usize| smooth(OFF_FLOOR, ON_FLOOR, src.contact[k].z - s.flesh(k, src) - floor);
    // WHAT CARRIES HIM: a knee, the seat, the chest or the head of the figure's on the floor (kneeling, sitting, lying, on all
    // fours) carries his body there too, and his hands and feet are brought to the floor by his limbs (his arms and legs have
    // their own lengths: Goofy's long arms, Donald's short legs); else (standing, crouching) his lowest part rests on the floor
    let trunk = TRUNK.iter().any(|k| on(*k) > 0.5);
    let (low, tgt_part) = if trunk { TRUNK.iter().map(|k| (tgt.contact[*k].z - t.flesh(*k, tgt), *k)).fold((f32::MAX, 0), |a, b| if b.0 < a.0 { b } else { a }) } else { t.rest(tgt) };
    // THE CROUCH: a hand of the figure's on the floor that his shorter arm cannot reach from where he stands (an item picked up)
    // lowers his hips, his planted feet staying on the floor (their goals below), never past his knees' fold
    let rested = tgt.moved(Vec3::Z * (floor - low));
    let mut crouch = 0.0f32;
    if !trunk {
        for (k, part) in HANDS.iter().enumerate() {
            let h = rested.contact[*part];
            let short = (Vec3::new(h.x, h.y, floor + t.flesh(*part, &rested)) - rested.shoulder[k]).length() - 0.97 * t.arm(k);
            crouch = crouch.max(short.max(0.0) * on(*part));
        }
        let hips = 0.5 * (rested.hip[0].z + rested.hip[1].z) - floor - t.dims.ankle_h;
        // (and no deeper than his planted feet can follow: their goals below)
        crouch = crouch.min((hips - CROUCH_MIN_HIPS * 0.5 * (t.leg(0) + t.leg(1))).max(0.0)).min(0.95 * FOOT_FIX_SHARE * t.leg(0).min(t.leg(1)));
        // (nor his seat, chest, head or a knee into the floor: only the feet are brought back up)
        let clear = TRUNK.iter().map(|k| rested.contact[*k].z - t.flesh(*k, &rested) - floor).fold(f32::MAX, f32::min);
        crouch = crouch.min(clear.max(0.0));
    }
    let lift = floor - low - crouch;
    let tg = tgt.moved(Vec3::Z * lift);
    let mut limbs: [Option<Goal>; 4] = [None; 4];
    // THE HANDS: on the floor where the figure's are (or wherever his would go through it); else at what they hold, within reach
    for (k, limb) in limbs.iter_mut().enumerate().take(2) {
        let part = HANDS[k];
        let h = tg.contact[part];
        let under = smooth(0.0, -UNDER_FLOOR, h.z - t.flesh(part, &tg) - floor);
        let w = on(part).max(under);
        if w > 0.0 {
            *limb = Some(Goal { to: Vec3::new(h.x, h.y, floor + t.flesh(part, &tg)), weight: w });
            continue;
        }
        if t.arm(k) < PROP_ARM_RATIO * s.arm(k) || touches_itself(src, k) {
            continue;
        }
        let to = src.contact[part];
        let w = smooth(1.15 * t.arm(k), 0.98 * t.arm(k), (to - tg.shoulder[k]).length());
        if w > 0.0 {
            *limb = Some(Goal { to, weight: w });
        }
    }
    // THE FEET: a sole of the figure's on the floor is on it for him too (or wherever his would go through it)
    for j in 0..2 {
        let sole_s = (src.contact[ANKLES[j]].z - s.flesh(ANKLES[j], src)).min(src.contact[TOES[j]].z) - floor;
        let sole_t = (tg.contact[ANKLES[j]].z - t.flesh(ANKLES[j], &tg)).min(tg.contact[TOES[j]].z) - floor;
        let w = smooth(OFF_FLOOR, ON_FLOOR, sole_s).max(smooth(0.0, -UNDER_FLOOR, sole_t));
        if w > 0.0 && sole_t.abs() > 0.05 && sole_t.abs() < FOOT_FIX_SHARE * t.leg(j) {
            limbs[2 + j] = Some(Goal { to: tg.contact[ANKLES[j]] - Vec3::Z * sole_t, weight: w });
        }
    }
    Fit { floor, lift, rests_on: (src_part, tgt_part), limbs }
}

/// The names of the parts (for the log and the proof).
pub fn part_name(part: usize) -> &'static str {
    ["seat", "chest", "head", "left hand", "right hand", "left knee", "right knee", "left sole", "right sole", "left toes", "right toes"].get(part).copied().unwrap_or("?")
}

/// The measures of the figure (the Tarnished: its bind pose, every `ROWS` row, KH3 axes).
pub fn tarnished(binds: &[(Vec3, Quat)]) -> Option<Body> {
    Dims::of_binds(binds).map(|dims| Body { dims, depth: TARNISHED_DEPTH })
}

/// Places of a component-space pose by bone name, in the world.
pub fn world_points(skeleton: &kh_anim::Skeleton, component: &[kh_anim::Trs], to_world: &kh_anim::Trs) -> Option<Points> {
    Points::of_names(&|n: &str| skeleton.index_of(n).and_then(|k| component.get(k)).map(|c| to_world.transform_point(c.translation)))
}

/// A KH skeleton's measures (its reference pose).
pub fn body_of_skeleton(skeleton: &kh_anim::Skeleton, depth: f32) -> Option<Body> {
    let c = kh_anim::pose::component_space(skeleton, &skeleton.reference_pose());
    Body::of_named(&|n: &str| skeleton.index_of(n).and_then(|k| c.get(k)).map(|t| t.translation), depth)
}

/// The depth of each leader's body (cm): ANIMS' profiles (propfit.rs `Profile::depth`).
pub const SORA_DEPTH: f32 = 16.0;
#[cfg(any(test, feature = "party"))]
pub fn depth_of(chara: kh_host::party::PartyChara) -> f32 {
    match chara {
        kh_host::party::PartyChara::Sora => SORA_DEPTH,
        kh_host::party::PartyChara::Donald => 20.0,
        kh_host::party::PartyChara::Goofy => 15.0,
    }
}

/// THE WHOLE STEP ON A KH SKELETON (Sora without an outfit, Donald, Goofy): the retarget at the figure's pelvis, measured,
/// again at the fitted pelvis, each limb to its goal (a two-bone solve, its end keeps its turn). `rows`: the retarget's rows
/// of `source`; `source`: every `ROWS` row (world). Returns the fit (None: a part is missing: the plain retarget is left).
pub fn apply_skeleton(retarget: &kh_anim::retarget::Retarget, skeleton: &kh_anim::Skeleton, rows: &[(Vec3, Quat)], source: &[(Vec3, Quat)], (s, t): (&Body, &Body), to_world: &kh_anim::Trs, local: &mut [kh_anim::Trs]) -> Option<Fit> {
    let pelvis = source.first()?.0;
    retarget.apply(skeleton, rows, pelvis, to_world, 1.0, local);
    let src = Points::of_rows(source)?;
    let tgt = world_points(skeleton, &kh_anim::pose::component_space(skeleton, local), to_world)?;
    let f = fit(&src, s, &tgt, t);
    retarget.apply(skeleton, rows, pelvis + Vec3::Z * f.lift, to_world, 1.0, local);
    for (k, goal) in f.limbs.iter().enumerate() {
        let Some(goal) = goal else { continue };
        let names = crate::limbs::LIMBS[k];
        if let (Some(u), Some(l), Some(e)) = (skeleton.index_of(names[0]), skeleton.index_of(names[1]), skeleton.index_of(names[2])) {
            kh_anim::retarget::reach_arm(skeleton, u, l, e, to_world.rotation.conjugate() * (goal.to - to_world.translation), goal.weight, local);
        }
    }
    Some(f)
}

/// THE WHOLE STEP FOR SORA IN A WORN OUTFIT (KH1, KH2, the worlds' outfits): measured on the OUTFIT'S skeleton (its pose from
/// his, kh-er-render outfit.rs), his pelvis moved so that the OUTFIT rests on the floor (its translations are his scaled by
/// `Retarget::scale`), each limb solved on the outfit's skeleton and written back into his (limbs.rs `reach_outfit`).
#[cfg(windows)]
pub fn apply_outfit(retarget: &kh_anim::retarget::Retarget, skeleton: &kh_anim::Skeleton, rows: &[(Vec3, Quat)], source: &[(Vec3, Quat)], (s, worn): (&Body, &kh_er_render::outfit::Retarget), to_world: &kh_anim::Trs, local: &mut [kh_anim::Trs]) -> Option<Fit> {
    let pelvis = source.first()?.0;
    let t = outfit_body(worn, skeleton)?;
    retarget.apply(skeleton, rows, pelvis, to_world, 1.0, local);
    let src = Points::of_rows(source)?;
    let (_, oc) = worn.pose_full(local, &[]);
    let tgt = Points::of_names(&|n: &str| worn.index_of(n).and_then(|k| oc.get(k)).map(|c| to_world.transform_point(c.translation)))?;
    let f = fit(&src, s, &tgt, &t);
    let scale = if worn.scale > 0.05 { worn.scale } else { 1.0 };
    retarget.apply(skeleton, rows, pelvis + Vec3::Z * (f.lift / scale), to_world, 1.0, local);
    for (k, goal) in f.limbs.iter().enumerate() {
        if let Some(goal) = goal {
            crate::limbs::reach_outfit(worn, local, k, to_world.rotation.conjugate() * (goal.to - to_world.translation), goal.weight);
        }
    }
    Some(f)
}

/// The worn outfit's measures (its reference = its pose for his reference pose), cached per outfit retarget.
#[cfg(windows)]
pub fn outfit_body(worn: &kh_er_render::outfit::Retarget, host: &kh_anim::Skeleton) -> Option<Body> {
    static CACHE: std::sync::Mutex<Option<(usize, usize, Body)>> = std::sync::Mutex::new(None);
    let key = (worn as *const _ as usize, worn.len());
    if let Ok(g) = CACHE.lock() {
        if let Some((a, n, b)) = *g {
            if (a, n) == key {
                return Some(b);
            }
        }
    }
    let (_, oc) = worn.pose_full(&host.reference_pose(), &[]);
    let body = Body::of_named(&|n: &str| worn.index_of(n).and_then(|k| oc.get(k)).map(|c| c.translation), SORA_DEPTH * worn.scale)?;
    if let Ok(mut g) = CACHE.lock() {
        *g = Some((key.0, key.1, body));
    }
    Some(body)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A standing figure: feet on z = 0, the measures of a 180 cm human (Tarnished-like).
    fn standing(scale: f32, lift: f32) -> (Points, Body) {
        let v = |x: f32, y: f32, z: f32| Vec3::new(x, y, z) * scale + Vec3::Z * lift;
        let p = Points {
            contact: [v(0.0, 0.0, 97.0), v(0.0, 0.0, 130.0), v(0.0, 0.0, 160.0), v(-23.0, 0.0, 85.0), v(23.0, 0.0, 85.0), v(-10.0, 2.0, 52.0), v(10.0, 2.0, 52.0), v(-10.0, 0.0, 11.0), v(10.0, 0.0, 11.0), v(-10.0, 12.0, 0.0), v(10.0, 12.0, 0.0)],
            shoulder: [v(-18.0, 0.0, 139.0), v(18.0, 0.0, 139.0)],
            hip: [v(-10.0, 0.0, 97.0), v(10.0, 0.0, 97.0)],
        };
        let dims = Dims { pelvis_h: 97.0 * scale, ankle_h: 11.0 * scale, hip_h: 97.0 * scale, hip_w: 10.0 * scale, head_h: 175.0 * scale, shoulder_h: 139.0 * scale, shoulder_w: 18.0 * scale, limb: [51.0 * scale, 51.0 * scale, 86.0 * scale, 86.0 * scale], root: [40.0 * scale; 4] };
        (p, Body { dims, depth: 14.0 * scale })
    }

    #[test]
    fn a_shorter_body_standing_rests_on_the_figure_s_floor() {
        let (src, s) = standing(1.0, 300.0);
        // his pose made with his pelvis at the figure's: floating (his legs are half as long)
        let (tgt, t) = standing(0.5, 300.0 + 48.5);
        let f = fit(&src, &s, &tgt, &t);
        assert!((f.floor - 300.0).abs() < 1.0e-3, "{}", f.floor);
        assert!((f.lift + 48.5).abs() < 1.0e-3, "{}", f.lift);
        assert!((7..=10).contains(&f.rests_on.0) && (7..=10).contains(&f.rests_on.1), "on the feet: {:?}", f.rests_on);
        // both soles on the floor already: no foot goal; hands hang (no contact): Donald-short arms never reach for them
        assert!(f.limbs.iter().all(Option::is_none), "{:?}", f.limbs);
    }

    #[test]
    fn a_figure_lying_rests_on_its_chest_and_he_on_his() {
        let (src, s) = standing(1.0, 0.0);
        // lay it face down: up (z) -> forward (y), the chest's flesh under it
        let lay = |p: Vec3| Vec3::new(p.x, p.z, -p.y);
        let mut src_l = Points { contact: src.contact.map(lay), shoulder: src.shoulder.map(lay), hip: src.hip.map(lay) };
        let flesh = s.flesh(CHEST, &src);
        let low = src_l.contact.iter().map(|p| p.z).fold(f32::MAX, f32::min);
        src_l = src_l.moved(Vec3::Z * (flesh - low + 50.0));
        let (floor, part) = s.rest(&src_l);
        assert!(floor.is_finite() && TRUNK.iter().any(|k| src_l.contact[*k].z - s.flesh(*k, &src_l) - floor < ON_FLOOR), "the trunk lies on the floor ({})", part_name(part));
        let (tgt, t) = standing(0.6, 0.0);
        let tgt_l = Points { contact: tgt.contact.map(lay), shoulder: tgt.shoulder.map(lay), hip: tgt.hip.map(lay) }.moved(Vec3::Z * 200.0);
        let f = fit(&src_l, &s, &tgt_l, &t);
        let lain = tgt_l.moved(Vec3::Z * f.lift);
        let low_t = TRUNK.iter().map(|k| lain.contact[*k].z - t.flesh(*k, &lain)).fold(f32::MAX, f32::min);
        assert!((low_t - floor).abs() < 1.0e-3, "his trunk lies on the figure's floor: {low_t} / {floor}");
        assert!(TRUNK.contains(&f.rests_on.1), "{:?}", f.rests_on);
        // (nothing of his under the floor that a limb goal does not bring up)
        let (all_low, k) = t.rest(&lain);
        assert!(all_low > floor - 1.0e-3 || f.limbs.iter().flatten().count() > 0, "{} under the floor", part_name(k));
    }

    #[test]
    fn hands_on_the_floor_and_props_only_within_reach() {
        let (mut src, s) = standing(1.0, 0.0);
        // the left hand pushes on the floor in front, the right holds an item away from the body at chest height
        src.contact[HANDS[0]] = Vec3::new(-25.0, 40.0, 0.0);
        src.contact[HANDS[0]].z = s.flesh(HANDS[0], &src);
        src.contact[HANDS[1]] = Vec3::new(22.0, 36.0, 122.0);
        let (tgt, t) = standing(0.9, 0.0);
        let f = fit(&src, &s, &tgt, &t);
        let l = f.limbs[0].expect("left hand on the floor");
        let crouched = tgt.moved(Vec3::Z * f.lift);
        assert!((l.weight - 1.0).abs() < 1.0e-3 && (l.to.z - t.flesh(HANDS[0], &crouched)).abs() < 1.0e-3);
        // (the item alone: standing, his hand at it)
        let mut item = src;
        item.contact[HANDS[0]] = Vec3::new(-23.0, 0.0, 85.0);
        let f = fit(&item, &s, &tgt, &t);
        let r = f.limbs[1].expect("right hand at the item");
        assert!((r.to - src.contact[HANDS[1]]).length() < 1.0e-3 && r.weight > 0.99);
        // Donald-short arms: no prop reach (the floor still counts)
        let (tgt_d, t_d) = standing(0.35, 0.0);
        let f = fit(&src, &s, &tgt_d, &t_d);
        assert!(f.limbs[0].is_some() && f.limbs[1].is_none(), "{:?}", f.limbs);
        // a hand on its own thigh is no world contact
        let mut sit = src;
        sit.contact[HANDS[1]] = Vec3::new(10.0, 10.0, 75.0);
        assert!(touches_itself(&sit, 1));
    }

    #[test]
    fn a_planted_foot_is_put_on_the_floor_when_the_other_carries_him() {
        let (src, s) = standing(1.0, 0.0);
        let (mut tgt, t) = standing(0.8, 0.0);
        // his right leg a little short in this pose: its sole 3 cm over the floor
        for k in [ANKLES[1], TOES[1]] {
            tgt.contact[k].z += 3.0;
        }
        let f = fit(&src, &s, &tgt, &t);
        assert!(f.lift.abs() < 1.0e-3);
        let g = f.limbs[3].expect("the right foot down");
        assert!((g.to.z - (tgt.contact[ANKLES[1]].z - 3.0)).abs() < 1.0e-3 && (g.weight - 1.0).abs() < 1.0e-3);
        assert!(f.limbs[2].is_none());
    }

    #[test]
    fn a_short_arm_crouches_to_a_hand_on_the_floor_with_his_feet_kept_on_it() {
        let (mut src, s) = standing(1.0, 0.0);
        // the figure's right hand on the floor in front (an item picked up)
        src.contact[HANDS[1]] = Vec3::new(20.0, 30.0, 0.0);
        src.contact[HANDS[1]].z = s.flesh(HANDS[1], &src);
        // his arms are short for his height: from his shoulder (125 cm) the floor is out of reach standing
        let (tgt, mut t) = standing(0.9, 0.0);
        t.dims.limb[0] = 30.0;
        t.dims.limb[1] = 30.0;
        let f = fit(&src, &s, &tgt, &t);
        let hips = 0.5 * (tgt.hip[0].z + tgt.hip[1].z) - t.dims.ankle_h;
        assert!(f.lift < -1.0, "he crouches: {}", f.lift);
        assert!(hips + f.lift >= CROUCH_MIN_HIPS * t.leg(0) - 1.0e-3, "never past his knees' fold");
        for (goal, ankle) in f.limbs[2..].iter().zip(ANKLES) {
            let g = goal.expect("each planted foot kept on the floor");
            assert!((g.to.z - tgt.contact[ankle].z).abs() < 1.0e-3, "the soles where they rest: {:?}", g.to);
        }
        assert!(f.limbs[1].is_some_and(|g| (g.to.z - t.flesh(HANDS[1], &tgt.moved(Vec3::Z * f.lift))).abs() < 1.0e-3));
    }

    #[test]
    fn the_weights_are_continuous() {
        // a hand lowered to the floor: its weight rises smoothly, never jumps
        let (src, s) = standing(1.0, 0.0);
        let (tgt, t) = standing(0.9, 0.0);
        let mut last = 0.0;
        for i in 0..=100 {
            let mut p = src;
            p.contact[HANDS[0]] = Vec3::new(-25.0, 60.0, s.flesh(HANDS[0], &src) + 10.0 - i as f32 * 0.1);
            let w = fit(&p, &s, &tgt, &t).limbs[0].map_or(0.0, |g| g.weight);
            assert!((w - last).abs() < 0.08, "step {i}: {last} -> {w}");
            last = w;
        }
        assert!((last - 1.0).abs() < 1.0e-3);
    }
}
