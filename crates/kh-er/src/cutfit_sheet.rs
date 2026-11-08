//! (CUTSCENE-ANIM 2026-11-06) THE OFFLINE PROOF: Elden Ring's cutscene figure of the player retargeted onto every leader
//! (Sora in KH3's look, Sora in the KH1 outfit, Donald, Goofy), BEFORE (the rule as it was: figure.rs `posed` / leader.rs
//! `PoseOver`) and AFTER (cutfit.rs), measured, and drawn with their own meshes.
//!
//! THE SOURCES (KH3 axes, cm):
//! - REAL, read in the game by IMPL_ER16 (`dbg.figskel`): build/sora_tmp/er/er16/figskel_b.txt = THE CUTSCENE'S FIGURE (c0000
//!   act of the wake-up scene after the Grafted Scion's scripted death, Torrent and Melina: the figure lying face down), and
//!   figskel_p.txt = the real player standing in play (its bind pose is every source's bind);
//! - MADE from that bind by forward kinematics and a two-bone solve per limb, for the other cutscene pose classes: rising on all
//!   fours, kneeling on one knee, sitting on the ground (Melina at a grace), an item received in both hands, an item picked up off
//!   the floor, a boss intro's ready stance. The remo tracks themselves (Havok tagfile, spline-compressed) are not readable
//!   offline: no decoder exists in the tree.
//!
//! Run: `cargo test -p kh-er --lib cutfit_sheet -- --nocapture` (pictures to `KH_PROOF_DIR`, default
//! build/sora_tmp/weapons/CUTSCENE-ANIM/shots); skipped without the files.

use glam::{Mat4, Quat, Vec3};
use kh_anim::{Skeleton, Trs};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use crate::cutfit::{self, Body, Points};
use crate::figure::ROWS;

fn project() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn er_matrix(f: &[f32]) -> (Vec3, Quat) {
    let m = glam::Mat3::from_cols(Vec3::new(f[0], f[4], f[8]).normalize_or_zero(), Vec3::new(f[1], f[5], f[9]).normalize_or_zero(), Vec3::new(f[2], f[6], f[10]).normalize_or_zero());
    (Vec3::new(f[3], f[7], f[11]), Quat::from_mat3(&m).normalize())
}

/// A `dbg.figskel` file: per node (index, name, parent, model-space bind matrix) and the pose (one world matrix per node).
struct Figskel {
    nodes: std::collections::HashMap<String, (usize, i64, Vec<f32>)>,
    names: std::collections::HashMap<usize, String>,
    pose: Vec<Vec<f32>>,
}

fn read_figskel(file: &str) -> Option<Figskel> {
    let text = std::fs::read_to_string(project().join("build/sora_tmp/er/er16").join(file)).ok()?;
    let mut f = Figskel { nodes: Default::default(), names: Default::default(), pose: Vec::new() };
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("node ") {
            let (idx, rest) = rest.split_once(' ')?;
            let idx: usize = idx.parse().ok()?;
            let name = rest.split('"').nth(1)?.to_string();
            let parent: i64 = rest.split("links [").nth(1)?.split(',').next()?.trim().parse().ok()?;
            let b: Vec<f32> = rest.split(" B [").nth(1)?.split(']').next()?.split(',').filter_map(|v| v.trim().parse().ok()).collect();
            if b.len() == 12 {
                f.names.insert(idx, name.clone());
                f.nodes.entry(name).or_insert((idx, parent, b));
            }
        } else if let Some(rest) = line.strip_prefix("pose ") {
            let v: Vec<f32> = rest.split('[').nth(1)?.split(']').next()?.split(',').filter_map(|v| v.trim().parse().ok()).collect();
            f.pose.push(v);
        }
    }
    Some(f)
}

/// The rows' bind (KH3 axes) and each row's parent row.
/// The rows' bind and each row's parent row.
type Bind = (Vec<(Vec3, Quat)>, Vec<Option<usize>>);

fn bind_of(f: &Figskel) -> Option<Bind> {
    let mut bind = Vec::new();
    for r in ROWS.iter() {
        let (_, _, b) = f.nodes.get(r.source)?;
        let (p, q) = er_matrix(b);
        bind.push((crate::coords::dir_er_to_kh(p), crate::figure::rotation_kh(q)));
    }
    let row_of = |name: &str| ROWS.iter().position(|r| r.source == name);
    let mut parent = Vec::new();
    for (k, r) in ROWS.iter().enumerate() {
        let (mut at, _, _) = f.nodes[r.source];
        let mut found = None;
        for _ in 0..64 {
            let p = f.nodes.get(f.names.get(&at)?).map_or(-1, |n| n.1);
            if p < 0 {
                break;
            }
            at = p as usize;
            if let Some(row) = f.names.get(&at).and_then(|n| row_of(n)) {
                found = Some(row);
                break;
            }
        }
        parent.push(if k == 0 { None } else { found.or(Some(0)) });
    }
    Some((bind, parent))
}

/// The figure's pose of the file (world matrices of the rows' nodes), KH3 axes.
fn pose_of(f: &Figskel) -> Option<Vec<(Vec3, Quat)>> {
    ROWS.iter()
        .map(|r| {
            let (idx, _, _) = f.nodes.get(r.source)?;
            let m = f.pose.get(*idx).filter(|m| m.len() == 12)?;
            let (p, q) = er_matrix(m);
            Some((crate::coords::dir_er_to_kh(p), crate::figure::rotation_kh(q)))
        })
        .collect()
}

/// A pose put facing +Y, its pelvis over the origin, its lowest row bone at z = 0.
fn normalised(pose: &[(Vec3, Quat)]) -> Vec<(Vec3, Quat)> {
    let flat = |v: Vec3| Vec3::new(v.x, v.y, 0.0);
    let toes = flat((pose[17].0 - pose[16].0) + (pose[21].0 - pose[20].0));
    let fwd = if (pose[5].0 - pose[0].0).normalize_or_zero().z < 0.5 { flat(pose[5].0 - pose[0].0) } else { toes };
    let turn = Quat::from_rotation_arc(fwd.normalize_or(Vec3::Y), Vec3::Y);
    let low = pose.iter().map(|(p, _)| p.z).fold(f32::MAX, f32::min);
    let pelvis = pose[0].0;
    pose.iter().map(|(p, q)| (turn * (*p - Vec3::new(pelvis.x, pelvis.y, low)), (turn * *q).normalize())).collect()
}

/// The made poses: the bind, its rows' tree, turned and solved.
struct Maker {
    bind: Vec<(Vec3, Quat)>,
    parent: Vec<Option<usize>>,
}

impl Maker {
    fn subtree(&self, k: usize) -> Vec<usize> {
        let mut out = vec![k];
        let mut i = 0;
        while i < out.len() {
            let at = out[i];
            out.extend(self.parent.iter().enumerate().filter(|(_, p)| **p == Some(at)).map(|(c, _)| c));
            i += 1;
        }
        out
    }

    fn turn(&self, pose: &mut [(Vec3, Quat)], k: usize, r: Quat) {
        let about = pose[k].0;
        for c in self.subtree(k) {
            pose[c].0 = about + r * (pose[c].0 - about);
            pose[c].1 = (r * pose[c].1).normalize();
        }
    }

    /// The bind (facing +Y, toes on z = 0) with the pelvis moved by `pelvis`, the trunk leaned forward by `lean` degrees, the
    /// knees bent forward first (a solve keeps the side a joint is bent to), each limb's end at its goal.
    fn pose(&self, pelvis: Vec3, lean: f32, goals: [Option<Vec3>; 4]) -> Vec<(Vec3, Quat)> {
        let mut p = self.bind.clone();
        for v in p.iter_mut() {
            v.0 += pelvis;
        }
        if lean != 0.0 {
            self.turn(&mut p, 1, Quat::from_rotation_x(-lean.to_radians()));
        }
        for (j, leg) in [crate::propfit::rows::LEG_L, crate::propfit::rows::LEG_R].iter().enumerate() {
            self.turn(&mut p, leg[1], Quat::from_rotation_x((-25.0f32).to_radians()));
            if goals[2 + j].is_some_and(|g| g.y - p[leg[0]].0.y > 20.0) {
                // (a foot far in front: the thigh raised above the hip-to-foot line first, so that the knee comes UP)
                let low = goals[2 + j].is_some_and(|g| p[leg[0]].0.z - g.z < 40.0);
                self.turn(&mut p, leg[0], Quat::from_rotation_x(if low { 115.0f32 } else { 70.0 }.to_radians()));
            }
            // (a knee to the floor behind: the thigh down and back first)
            if goals[2 + j].is_some_and(|g| g.y - p[leg[0]].0.y < -20.0 && g.z < 15.0) {
                self.turn(&mut p, leg[0], Quat::from_rotation_x((-20.0f32).to_radians()));
            }
        }
        // (the arms brought forward from the A pose before their solve: an elbow bends down, not up)
        for arm in [crate::propfit::rows::ARM_L, crate::propfit::rows::ARM_R] {
            self.turn(&mut p, arm[1], Quat::from_rotation_x(35.0f32.to_radians()));
        }
        for (k, goal) in goals.iter().enumerate() {
            let Some(goal) = goal else { continue };
            let [a, b, c] = crate::propfit::rows::LIMBS[k];
            for _ in 0..3 {
                let Some((q1, q2)) = crate::limbs::two_bone(p[a].0, p[b].0, p[c].0, *goal) else { break };
                self.turn(&mut p, a, q1);
                self.turn(&mut p, b, q2);
            }
        }
        // (the feet as the bind's: flat on their soles, toes forward along the leg's heading; a foot of the game's lies flat)
        for (j, leg) in [crate::propfit::rows::LEG_L, crate::propfit::rows::LEG_R].iter().enumerate() {
            if goals[2 + j].is_none() {
                continue;
            }
            let (foot, toe) = (leg[2], if j == 0 { crate::propfit::rows::TOE_L } else { crate::propfit::rows::TOE_R });
            let want = self.bind[toe].0 - self.bind[foot].0;
            let now = p[toe].0 - p[foot].0;
            // (only for a foot under its knee: a shin along the floor keeps the foot as the leg turned it)
            if (p[leg[1]].0 - p[foot].0).normalize_or_zero().z > 0.6 {
                self.turn(&mut p, foot, Quat::from_rotation_arc(now.normalize_or_zero(), want.normalize_or_zero()));
            } else if p[toe].0.z < 0.0 {
                // (a shin along the floor: the foot pitched up until its toes rest on the floor, not in it)
                let d = now;
                let dz = -p[foot].0.z;
                let fwd = (d.length_squared() - d.x * d.x - dz * dz).max(0.0).sqrt() * if d.y < 0.0 { -1.0 } else { 1.0 };
                self.turn(&mut p, foot, Quat::from_rotation_arc(d.normalize_or_zero(), Vec3::new(d.x, fwd, dz).normalize_or_zero()));
            }
        }
        p
    }
}

/// One cutscene pose class.
struct Scene {
    name: &'static str,
    title: &'static str,
    source: Vec<(Vec3, Quat)>,
}

fn scenes(maker: &Maker, real_stand: Option<Vec<(Vec3, Quat)>>, real_lying: Option<Vec<(Vec3, Quat)>>) -> Vec<Scene> {
    let v = Vec3::new;
    let mut out = Vec::new();
    if let Some(s) = real_lying {
        out.push(Scene { name: "real_wakeup_lying", title: "REAL ER CUTSCENE FRAME: the wake-up after the Grafted Scion (Torrent / Melina scene), the figure lying face down (er16/figskel_b)", source: normalised(&s) });
    }
    if let Some(s) = real_stand {
        out.push(Scene { name: "real_wakeup_body", title: "REAL ER POSE, the same scene: the hidden player's own body lying curled (er16/figskel_p)", source: normalised(&s) });
    }
    out.push(Scene { name: "rise_all_fours", title: "rising from the ground: knees and hands on the floor (the wake-up's push up)", source: maker.pose(v(0.0, -5.0, -53.0), 95.0, [Some(v(20.0, 32.0, 3.0)), Some(v(-20.0, 32.0, 3.0)), Some(v(12.0, -40.0, 9.0)), Some(v(-12.0, -40.0, 9.0))]) });
    out.push(Scene { name: "kneel_one_knee", title: "kneeling on one knee (a grace, an oath, an item from a kneeling NPC)", source: maker.pose(v(0.0, -8.0, -42.0), 8.0, [Some(v(14.0, 30.0, 58.0)), None, Some(v(12.0, 32.0, 11.0)), Some(v(-12.0, -42.0, 6.0))]) });
    out.push(Scene { name: "sit_ground_grace", title: "sitting on the ground at a grace, knees up, hands on the knees (Melina's offer)", source: maker.pose(v(0.0, -10.0, -84.0), -12.0, [Some(v(16.0, 32.0, 40.0)), Some(v(-16.0, 32.0, 40.0)), Some(v(14.0, 48.0, 11.0)), Some(v(-14.0, 48.0, 11.0))]) });
    out.push(Scene { name: "receive_item", title: "an item received in both hands, standing (Melina's whistle, a gift)", source: maker.pose(v(0.0, 0.0, -2.0), 10.0, [Some(v(6.0, 38.0, 112.0)), Some(v(-6.0, 38.0, 112.0)), Some(v(11.0, 4.0, 11.0)), Some(v(-11.0, -4.0, 11.0))]) });
    out.push(Scene { name: "pickup_floor", title: "an item picked up off the floor, crouched", source: maker.pose(v(0.0, -12.0, -60.0), 85.0, [None, Some(v(-12.0, 30.0, 4.0)), Some(v(14.0, 22.0, 11.0)), Some(v(-14.0, -20.0, 11.0))]) });
    out.push(Scene { name: "boss_intro_ready", title: "a boss intro's ready stance: feet apart, knees bent, a hand forward", source: maker.pose(v(0.0, -4.0, -12.0), 12.0, [Some(v(26.0, 10.0, 80.0)), Some(v(-18.0, 42.0, 100.0)), Some(v(20.0, 18.0, 11.0)), Some(v(-20.0, -22.0, 11.0))]) });
    out
}

/// A leader.
struct Target {
    name: &'static str,
    skeleton: Skeleton,
    base: Vec<Trs>,
    body: Body,
    retarget: kh_anim::retarget::Retarget,
    keep: Vec<usize>,
    /// The KH1 outfit drawn from Sora's skeleton.
    outfit: Option<std::sync::Arc<kh_er_render::outfit::Retarget>>,
    model: PathBuf,
    colour: Vec3,
}

fn targets(binds: &[(Vec3, Quat)]) -> Option<Vec<Target>> {
    let pack = project().join("build/sora_pack");
    let animator = kh_anim::Animator::new(kh_anim::Pack::load(&pack).ok()?, kh_anim::Settings::default()).ok()?;
    let sora = animator.skeleton().clone();
    let base = animator.sample(kh_core::AnimId::IdleN.sequences()[0], 0.0);
    let base = if base.len() == sora.len() { base } else { sora.reference_pose() };
    let (rs, ks) = crate::figure::retarget_from_binds(binds, &sora).ok()?;
    let sora_body = cutfit::body_of_skeleton(&sora, cutfit::SORA_DEPTH)?;
    let kh1 = kh_er_render::outfit::retarget_for(&pack, "kh1", &sora).ok().flatten().map(std::sync::Arc::new);
    let mut out = vec![Target { name: "Sora (KH3)", skeleton: sora.clone(), base: base.clone(), body: sora_body, retarget: rs.clone(), keep: ks.clone(), outfit: None, model: pack.join("sora.glb"), colour: Vec3::new(0.85, 0.25, 0.22) }];
    if let Some(r) = kh1 {
        let body = cutfit::outfit_body(&r, &sora)?;
        out.push(Target { name: "Sora (KH1 outfit)", skeleton: sora, base, body, retarget: rs, keep: ks, outfit: Some(r), model: kh_er_render::outfit::model_file(&pack, "kh1"), colour: Vec3::new(0.95, 0.6, 0.15) });
    }
    for (npc, name, chara, colour) in [("n_ex001", "Donald", kh_host::party::PartyChara::Donald, Vec3::new(0.25, 0.4, 0.9)), ("n_ex002", "Goofy", kh_host::party::PartyChara::Goofy, Vec3::new(0.3, 0.7, 0.3))] {
        let dir = pack.join("party").join(npc);
        let text = std::fs::read_to_string(dir.join("manifest.json")).ok()?;
        let skeleton = Skeleton::from_manifest(&kh_anim::json::Json::parse(text.trim_start_matches('\u{feff}')).ok()?).ok()?;
        let (retarget, keep) = crate::figure::retarget_from_binds(binds, &skeleton).ok()?;
        let body = cutfit::body_of_skeleton(&skeleton, cutfit::depth_of(chara))?;
        out.push(Target { name, base: skeleton.reference_pose(), skeleton, body, retarget, keep, outfit: None, model: dir.join("model.glb"), colour });
    }
    Some(out)
}

/// A leader's pose of a scene: the drawn skeleton's bones (component space) and its world place.
struct Drawn {
    /// The DRAWN skeleton's component pose (the outfit's for KH1) and the bone lookup.
    component: Vec<Trs>,
    points: Points,
    fit: Option<cutfit::Fit>,
}

fn drawn_points(t: &Target, local: &[Trs], to_world: &Trs) -> (Vec<Trs>, Points) {
    match &t.outfit {
        Some(r) => {
            let (_, oc) = r.pose_full(local, &[]);
            let p = Points::of_names(&|n: &str| r.index_of(n).and_then(|k| oc.get(k)).map(|c| to_world.transform_point(c.translation))).expect("the outfit's parts");
            (oc, p)
        }
        None => {
            let c = kh_anim::pose::component_space(&t.skeleton, local);
            let p = cutfit::world_points(&t.skeleton, &c, to_world).expect("the parts");
            (c, p)
        }
    }
}

/// BEFORE (the live rule until now: the pelvis bone's ratio about the lowest bone less 4 cm) or AFTER (cutfit.rs).
fn solve(t: &Target, src: &[(Vec3, Quat)], s: &Body, after: bool) -> Drawn {
    let rows: Vec<(Vec3, Quat)> = t.keep.iter().map(|k| src[*k]).collect();
    let low = src.iter().map(|(p, _)| p.z).fold(f32::MAX, f32::min) - 4.0;
    let to_world = Trs { translation: Vec3::new(src[0].0.x, src[0].0.y, low), rotation: Quat::IDENTITY, scale: Vec3::ONE };
    let mut local = t.base.clone();
    let fit = if after {
        match &t.outfit {
            Some(r) => cutfit::apply_outfit(&t.retarget, &t.skeleton, &rows, src, (s, r), &to_world, &mut local),
            None => cutfit::apply_skeleton(&t.retarget, &t.skeleton, &rows, src, (s, &t.body), &to_world, &mut local),
        }
    } else {
        let pelvis = t.retarget.pelvis_place(src[0].0, Vec3::Z, low);
        t.retarget.apply(&t.skeleton, &rows, pelvis, &to_world, 1.0, &mut local);
        None
    };
    let (component, points) = drawn_points(t, &local, &to_world);
    let component = component.iter().map(|c| c.then(&to_world)).collect();
    Drawn { component, points, fit }
}

/// The errors (cm) of a leader's pose against the figure's: (his rest off the floor, a planted sole off it, a hand on the floor
/// off it, a held item's hand off it).
fn errors(src: &Points, s: &Body, d: &Points, t: &Body) -> [f32; 4] {
    let (floor, _) = s.rest(src);
    let rest = (t.rest(d).0 - floor).abs();
    let mut sole = 0.0f32;
    let mut hand = 0.0f32;
    for j in 0..2 {
        let sole_s = (src.contact[7 + j].z - s.flesh(7 + j, src)).min(src.contact[9 + j].z) - floor;
        if sole_s < cutfit::ON_FLOOR {
            sole = sole.max(((d.contact[7 + j].z - t.flesh(7 + j, d)).min(d.contact[9 + j].z) - floor).abs());
        }
    }
    for k in 0..2 {
        if src.contact[3 + k].z - s.flesh(3 + k, src) - floor < cutfit::ON_FLOOR {
            hand = hand.max((d.contact[3 + k].z - t.flesh(3 + k, d) - floor).abs());
        }
    }
    let item = [3usize, 4].iter().map(|k| (d.contact[*k] - src.contact[*k]).length()).fold(0.0f32, f32::max);
    [rest, sole, hand, item]
}

// ---------------------------------------------------------------- the pictures: each leader's own mesh, posed, on the CPU

type Tri = ([Vec3; 3], [Vec3; 3], Vec3);

/// The leader's mesh skinned by his drawn pose (world), one flat colour.
fn skinned(model: &kh_er_render::model::Model, joint_of: &dyn Fn(&str) -> Option<Trs>, colour: Vec3) -> Vec<Tri> {
    let pack_to_component = kh_er_render::math::component_from_pack();
    let joints: Vec<Mat4> = model.joints.iter().map(|(name, inv)| joint_of(name).map_or(Mat4::IDENTITY, |t| kh_er_render::math::joint_in_pack(&t) * *inv)).collect();
    let mut out = Vec::new();
    for p in &model.primitives {
        let first = p.first_index as usize;
        for t in model.indices[first..first + p.index_count as usize].chunks(3) {
            let mut pos = [Vec3::ZERO; 3];
            let mut nor = [Vec3::ZERO; 3];
            for (k, &i) in t.iter().enumerate() {
                let v = &model.vertices[(i as i64 + p.base_vertex as i64) as usize];
                let mut skin = Mat4::ZERO;
                for c in 0..4 {
                    if v.weights[c] > 0.0 {
                        skin += joints.get(v.joints[c] as usize).copied().unwrap_or(Mat4::IDENTITY) * v.weights[c];
                    }
                }
                if v.weights.iter().sum::<f32>() <= 0.0 {
                    skin = Mat4::IDENTITY;
                }
                let to = pack_to_component * skin;
                pos[k] = to.transform_point3(Vec3::from(v.position));
                nor[k] = to.transform_vector3(Vec3::from(v.normal)).normalize_or_zero();
            }
            out.push((pos, nor, colour));
        }
    }
    out
}

const PW: usize = 330;
const PH: usize = 400;
const SS: usize = 2;

struct View {
    eye: Vec3,
    right: Vec3,
    up: Vec3,
    fwd: Vec3,
    focal: f32,
}

impl View {
    fn at(target: Vec3, dist: f32, above: f32) -> View {
        // from his left side and a little in front, `above` of the distance up (the figure faces +Y)
        let eye = target + Vec3::new(dist, dist * 0.35, dist * above);
        let fwd = (target - eye).normalize();
        let right = Vec3::Z.cross(fwd).normalize();
        let up = fwd.cross(right).normalize();
        View { eye, right, up, fwd, focal: 1.0 / (24.0f32.to_radians() * 0.5).tan() }
    }

    fn project(&self, p: Vec3, w: f32, h: f32) -> Option<Vec3> {
        let d = p - self.eye;
        let z = d.dot(self.fwd);
        (z > 1.0).then(|| Vec3::new(d.dot(self.right) / z * self.focal * h * 0.5 + w * 0.5, -d.dot(self.up) / z * self.focal * h * 0.5 + h * 0.5, z))
    }
}

/// One panel: the floor, the mesh, the figure's bones as a black line drawing over it, the floor line in red.
fn panel(tris: &[Tri], view: &View, floor: f32, centre: Vec3, figure: &[(Vec3, Vec3)]) -> Vec<Vec3> {
    let (w, h) = (PW * SS, PH * SS);
    let mut depth = vec![f32::MAX; w * h];
    let mut img = vec![Vec3::new(0.93, 0.94, 0.96); w * h];
    let sun = Vec3::new(0.4, 0.5, 0.75).normalize();
    // the floor: a square around the figure, two triangles
    let r = 160.0;
    let c = Vec3::new(centre.x, centre.y, floor);
    let q = [c + Vec3::new(-r, -r, 0.0), c + Vec3::new(r, -r, 0.0), c + Vec3::new(r, r, 0.0), c + Vec3::new(-r, r, 0.0)];
    let floor_tris: Vec<Tri> = vec![([q[0], q[1], q[2]], [Vec3::Z; 3], Vec3::new(0.62, 0.6, 0.56)), ([q[0], q[2], q[3]], [Vec3::Z; 3], Vec3::new(0.62, 0.6, 0.56))];
    for (pos, nor, colour) in floor_tris.iter().chain(tris.iter()) {
        let Some(s) = pos.iter().map(|p| view.project(*p, w as f32, h as f32)).collect::<Option<Vec<Vec3>>>() else { continue };
        let (a, b, cc) = (s[0], s[1], s[2]);
        let area = (b.x - a.x) * (cc.y - a.y) - (b.y - a.y) * (cc.x - a.x);
        if area.abs() < 1e-9 {
            continue;
        }
        let x0 = a.x.min(b.x).min(cc.x).floor().max(0.0) as usize;
        let x1 = (a.x.max(b.x).max(cc.x).ceil().max(0.0) as usize).min(w - 1);
        let y0 = a.y.min(b.y).min(cc.y).floor().max(0.0) as usize;
        let y1 = (a.y.max(b.y).max(cc.y).ceil().max(0.0) as usize).min(h - 1);
        for y in y0..=y1 {
            for x in x0..=x1 {
                let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                let w0 = ((b.x - px) * (cc.y - py) - (b.y - py) * (cc.x - px)) / area;
                let w1 = ((cc.x - px) * (a.y - py) - (cc.y - py) * (a.x - px)) / area;
                let w2 = 1.0 - w0 - w1;
                if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                    continue;
                }
                let z = w0 * a.z + w1 * b.z + w2 * cc.z;
                let at = y * w + x;
                if z >= depth[at] {
                    continue;
                }
                depth[at] = z;
                let n = (nor[0] * w0 + nor[1] * w1 + nor[2] * w2).normalize_or_zero();
                let n = if n.dot(view.eye - (pos[0] * w0 + pos[1] * w1 + pos[2] * w2)) < 0.0 { -n } else { n };
                img[at] = *colour * (0.35 + 0.65 * n.dot(sun).max(0.0));
            }
        }
    }
    // the figure's bones over everything (thin, dark), the floor's edge across the figure in red
    let mut line = |a: Vec3, b: Vec3, colour: Vec3, width: f32| {
        let (Some(pa), Some(pb)) = (view.project(a, w as f32, h as f32), view.project(b, w as f32, h as f32)) else { return };
        let n = ((pb - pa).truncate().length() * 2.0).ceil().max(1.0) as usize;
        for i in 0..=n {
            let p = pa.lerp(pb, i as f32 / n as f32);
            let r = width as i32;
            for dy in -r..=r {
                for dx in -r..=r {
                    let (x, y) = (p.x as i32 + dx, p.y as i32 + dy);
                    if x >= 0 && y >= 0 && (x as usize) < w && (y as usize) < h {
                        img[y as usize * w + x as usize] = colour;
                    }
                }
            }
        }
    };
    for (a, b) in figure {
        line(*a, *b, Vec3::new(0.05, 0.05, 0.05), 1.0);
    }
    line(c + Vec3::new(0.0, -r, 0.0), c + Vec3::new(0.0, r, 0.0), Vec3::new(0.85, 0.1, 0.1), 1.0);
    // downsample
    (0..PW * PH)
        .map(|i| {
            let (x, y) = (i % PW, i / PW);
            let mut acc = Vec3::ZERO;
            for sy in 0..SS {
                for sx in 0..SS {
                    acc += img[(y * SS + sy) * w + x * SS + sx];
                }
            }
            acc / (SS * SS) as f32
        })
        .collect()
}

/// The figure's bones as lines (its rows' tree).
fn figure_lines(src: &[(Vec3, Quat)], parent: &[Option<usize>]) -> Vec<(Vec3, Vec3)> {
    parent.iter().enumerate().filter_map(|(k, p)| p.map(|p| (src[p].0, src[k].0))).collect()
}

// ---------------------------------------------------------------- a PNG without a dependency (stored deflate blocks)

fn crc32(bytes: &[u8]) -> u32 {
    let mut c = 0xFFFF_FFFFu32;
    for b in bytes {
        c ^= u32::from(*b);
        for _ in 0..8 {
            c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
        }
    }
    !c
}

fn write_png(path: &Path, w: usize, h: usize, rgb: &[u8]) {
    let mut raw = Vec::with_capacity((w * 3 + 1) * h);
    for y in 0..h {
        raw.push(0);
        raw.extend_from_slice(&rgb[y * w * 3..(y + 1) * w * 3]);
    }
    let mut z = vec![0x78, 0x01];
    let chunks: Vec<&[u8]> = raw.chunks(65_535).collect();
    for (i, c) in chunks.iter().enumerate() {
        z.push(u8::from(i + 1 == chunks.len()));
        let n = c.len() as u16;
        z.extend_from_slice(&n.to_le_bytes());
        z.extend_from_slice(&(!n).to_le_bytes());
        z.extend_from_slice(c);
    }
    let (mut a, mut b) = (1u32, 0u32);
    for byte in &raw {
        a = (a + u32::from(*byte)) % 65_521;
        b = (b + a) % 65_521;
    }
    z.extend_from_slice(&((b << 16) | a).to_be_bytes());
    let mut out = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    let mut chunk = |kind: &[u8; 4], data: &[u8]| {
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        let mut body = kind.to_vec();
        body.extend_from_slice(data);
        out.extend_from_slice(&body);
        out.extend_from_slice(&crc32(&body).to_be_bytes());
    };
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&(w as u32).to_be_bytes());
    ihdr.extend_from_slice(&(h as u32).to_be_bytes());
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);
    chunk(b"IHDR", &ihdr);
    chunk(b"IDAT", &z);
    chunk(b"IEND", &[]);
    let _ = std::fs::write(path, out);
}

fn tone(c: Vec3) -> [u8; 3] {
    let f = |v: f32| (v.clamp(0.0, 1.0).powf(1.0 / 1.6) * 255.0 + 0.5) as u8;
    [f(c.x), f(c.y), f(c.z)]
}

#[test]
fn cutfit_sheet() {
    let (Some(p), Some(b)) = (read_figskel("figskel_p.txt"), read_figskel("figskel_b.txt")) else {
        eprintln!("cutfit_sheet: no er16 skeleton dumps: skipped");
        return;
    };
    let Some((bind, parent)) = bind_of(&p) else { panic!("the bind of figskel_p") };
    // (facing +Y, toes on z = 0, as propfit_sheet's)
    let fwd = {
        let f = (bind[17].0 - bind[16].0) + (bind[21].0 - bind[20].0);
        Vec3::new(f.x, f.y, 0.0).normalize()
    };
    let turn = Quat::from_rotation_arc(fwd, Vec3::Y);
    let floor = bind[17].0.z.min(bind[21].0.z);
    let pelvis = bind[0].0;
    let bind: Vec<(Vec3, Quat)> = bind.iter().map(|(p, q)| (turn * (*p - Vec3::new(pelvis.x, pelvis.y, floor)), (turn * *q).normalize())).collect();
    let Some(s) = cutfit::tarnished(&bind) else { panic!("the Tarnished's measures") };
    let Some(targets) = targets(&bind) else {
        eprintln!("cutfit_sheet: no pack: skipped");
        return;
    };
    let maker = Maker { bind: bind.clone(), parent: parent.clone() };
    let list = scenes(&maker, pose_of(&p), pose_of(&b));
    let out_dir = std::env::var("KH_PROOF_DIR").map(PathBuf::from).unwrap_or_else(|_| project().join("build/sora_tmp/weapons/CUTSCENE-ANIM/shots"));
    let pictures = std::env::var("KH_PROOF_PICS").map_or(true, |v| v != "0");
    let _ = std::fs::create_dir_all(&out_dir);
    let models: Vec<Option<kh_er_render::model::Model>> = targets.iter().map(|t| if pictures { kh_er_render::model::Model::load(&t.model).ok() } else { None }).collect();
    let mut report = String::new();
    let _ = writeln!(report, "CUTSCENE-ANIM offline proof: every leader on the cutscene figure's pose. Errors in cm, BEFORE -> AFTER: [his rest off the figure's floor, a planted sole off the floor, a hand on the floor off it, the farthest hand from the figure's hand]");
    let _ = writeln!(report, "The Tarnished: hips {:.1}, legs {:.1}, arms {:.1}, ankles {:.1}", s.dims.hip_h, s.dims.limb[2], s.dims.limb[0], s.dims.ankle_h);
    for t in &targets {
        let _ = writeln!(report, "{}: hips {:.1}, legs {:.1} ({:.2} of the Tarnished's; the retarget's pelvis ratio {:.2}), arms {:.1}, ankles {:.1}", t.name, t.body.dims.hip_h, t.body.dims.limb[2], t.body.dims.limb[2] / s.dims.limb[2], t.retarget.ratio, t.body.dims.limb[0], t.body.dims.ankle_h);
    }
    let mut worst_after = [0.0f32; 4];
    for scene in &list {
        let src = Points::of_rows(&scene.source).expect("the figure's parts");
        let (floor, rests) = s.rest(&src);
        let over = |k: usize| src.contact[k].z - s.flesh(k, &src) - floor;
        let _ = writeln!(report, "\n== {} ({}): the figure rests on its {} (floor {:.1}); its hands {:.1} / {:.1} and soles {:.1} / {:.1} over the floor", scene.name, scene.title, cutfit::part_name(rests), floor, over(3), over(4), over(7).min(src.contact[9].z - floor), over(8).min(src.contact[10].z - floor));
        let mut row_before = Vec::new();
        let mut row_after = Vec::new();
        let lines = figure_lines(&scene.source, &parent);
        let centre = scene.source.iter().fold(Vec3::ZERO, |a, (p, _)| a + *p) / scene.source.len() as f32;
        let extent = scene.source.iter().map(|(p, _)| (*p - centre).length()).fold(0.0f32, f32::max);
        let lying = (scene.source[5].0 - scene.source[0].0).normalize_or_zero().z < 0.5;
        let view = View::at(centre, 3.4 * extent + 140.0, if lying { 0.75 } else { 0.12 });
        for (ti, t) in targets.iter().enumerate() {
            let before = solve(t, &scene.source, &s, false);
            let after = solve(t, &scene.source, &s, true);
            let eb = errors(&src, &s, &before.points, &t.body);
            let ea = errors(&src, &s, &after.points, &t.body);
            for (w, e) in worst_after.iter_mut().zip(ea).take(3) {
                *w = w.max(e);
            }
            let fit = after.fit.map_or("no fit".to_string(), |f| format!("rests on his {}, lifted {:+.1}, goals [{}]", cutfit::part_name(f.rests_on.1), f.lift, f.limbs.iter().map(|g| g.map_or("-".to_string(), |g| format!("{:.2}", g.weight))).collect::<Vec<_>>().join(" ")));
            let _ = writeln!(report, "  {:<18} rest {:5.1} -> {:4.1} | soles {:5.1} -> {:4.1} | hands on floor {:5.1} -> {:4.1} | hand to figure's {:5.1} -> {:5.1} | {fit}", t.name, eb[0], ea[0], eb[1], ea[1], eb[2], ea[2], eb[3], ea[3]);
            // (the measured guarantees: his lowest part on the figure's floor, nothing NaN)
            assert!(ea[0] < 0.6, "{} / {}: rests {:.2} cm off the floor", scene.name, t.name, ea[0]);
            assert!(after.points.contact.iter().all(|p| p.is_finite()), "{} / {}: not finite", scene.name, t.name);
            if let Some(model) = models[ti].as_ref() {
                for (d, row) in [(&before, &mut row_before), (&after, &mut row_after)] {
                    let joint_of = |n: &str| -> Option<Trs> {
                        match &t.outfit {
                            Some(r) => r.index_of(n).and_then(|k| d.component.get(k)).copied(),
                            None => t.skeleton.index_of(n).and_then(|k| d.component.get(k)).copied(),
                        }
                    };
                    let tris = skinned(model, &joint_of, t.colour);
                    row.push(panel(&tris, &view, floor, centre, &lines));
                }
            }
        }
        if pictures && !row_after.is_empty() {
            let cols = row_after.len();
            let (w, h) = (PW * cols, PH * 2);
            let mut rgb = vec![255u8; w * h * 3];
            for (r, row) in [&row_before, &row_after].iter().enumerate() {
                for (c, img) in row.iter().enumerate() {
                    for y in 0..PH {
                        for x in 0..PW {
                            let px = if x == 0 || y == 0 { [40, 40, 40] } else { tone(img[y * PW + x]) };
                            let at = ((r * PH + y) * w + c * PW + x) * 3;
                            rgb[at..at + 3].copy_from_slice(&px);
                        }
                    }
                }
            }
            write_png(&out_dir.join(format!("{}.png", scene.name)), w, h, &rgb);
        }
    }
    let _ = writeln!(report, "\nWorst AFTER over every scene and leader: rest {:.2} cm, planted soles {:.2} cm, hands on the floor {:.2} cm", worst_after[0], worst_after[1], worst_after[2]);
    let _ = writeln!(report, "Pictures: one per scene, columns {}; top row BEFORE, bottom row AFTER; the figure's bones in black, the floor's line in red.", targets.iter().map(|t| t.name).collect::<Vec<_>>().join(", "));
    let _ = std::fs::write(out_dir.join("report.txt"), &report);
    eprintln!("{report}");
}
