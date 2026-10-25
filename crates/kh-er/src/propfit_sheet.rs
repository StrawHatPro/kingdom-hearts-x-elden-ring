//! (ANIMS 2026-10-23) THE OFFLINE PROOF: every borrowed action of the Tarnished retargeted onto Sora, Donald and Goofy,
//! BEFORE (the rule as it was) and AFTER (propfit.rs), with the contacts drawn and the errors measured.
//!
//! THE SOURCE: the real player's bind pose read in the game (build/sora_tmp/er/er16/figskel_p.txt, the 52 `ROWS` bones
//! of c0000), posed here by forward kinematics and a two-bone solve per limb onto each action's contacts: a double door
//! pushed, a single door pulled by its handle, a lever, the fog walked through (1.8 m in 3 s), a chest's lid, an item picked up, the grace
//! sat at (LADDER2: the ladder actions are gone with the climb). The Tarnished's real clips are not readable offline (Havok), so the poses are made to touch what the game's do.
//! THE TARGETS: the packs' own skeletons (build/sora_pack: Sora; party/n_ex001 Donald; party/n_ex002 Goofy) at their
//! own idle pose. Run: `cargo test -p kh-er --features party --lib propfit_sheet -- --nocapture` (writes the sheets to
//! `KH_PROOF_DIR`, default build/sora_tmp/party/ANIMS/proof); skipped without the files.

use glam::{Quat, Vec3};
use kh_anim::{Skeleton, Trs};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use crate::figure::ROWS;
use crate::propfit::{self, Dims, Fit, Kind, Profile};

const DT: f32 = 1.0 / 60.0;

thread_local! {
    /// The fallbacks to his own pose taken in the action being measured (reason per character).
    static FALLBACK: std::cell::RefCell<Vec<String>> = const { std::cell::RefCell::new(Vec::new()) };
}

fn project() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The Tarnished: the bind of every `ROWS` row (KH3 axes, cm, facing +Y, its toes on z = 0) and each row's parent row.
struct Source {
    bind: Vec<(Vec3, Quat)>,
    parent: Vec<Option<usize>>,
    /// The largest miss of a limb's goal by the source's own solve since the last `take_miss` (cm).
    miss: std::cell::Cell<f32>,
}

fn er_matrix(f: &[f32]) -> (Vec3, Quat) {
    let m = glam::Mat3::from_cols(Vec3::new(f[0], f[4], f[8]).normalize_or_zero(), Vec3::new(f[1], f[5], f[9]).normalize_or_zero(), Vec3::new(f[2], f[6], f[10]).normalize_or_zero());
    (Vec3::new(f[3], f[7], f[11]), Quat::from_mat3(&m).normalize())
}

fn read_source() -> Option<Source> {
    let text = std::fs::read_to_string(project().join("build/sora_tmp/er/er16/figskel_p.txt")).ok()?;
    let mut nodes: std::collections::HashMap<String, (usize, i64, Vec<f32>)> = std::collections::HashMap::new();
    let mut by_index: std::collections::HashMap<usize, String> = std::collections::HashMap::new();
    let mut parents: std::collections::HashMap<usize, i64> = std::collections::HashMap::new();
    for line in text.lines() {
        let Some(rest) = line.strip_prefix("node ") else { continue };
        let (idx, rest) = rest.split_once(' ')?;
        let idx: usize = idx.parse().ok()?;
        let name = rest.split('"').nth(1)?.to_string();
        let links = rest.split("links [").nth(1)?.split(']').next()?;
        let parent: i64 = links.split(',').next()?.trim().parse().ok()?;
        let b = rest.split(" B [").nth(1)?.split(']').next()?;
        let b: Vec<f32> = b.split(',').filter_map(|v| v.trim().parse().ok()).collect();
        if b.len() != 12 {
            continue;
        }
        by_index.insert(idx, name.clone());
        parents.insert(idx, parent);
        nodes.entry(name).or_insert((idx, parent, b));
    }
    let mut bind = Vec::new();
    for r in ROWS.iter() {
        let (_, _, b) = nodes.get(r.source)?;
        let (p, q) = er_matrix(b);
        bind.push((crate::coords::dir_er_to_kh(p), crate::figure::rotation_kh(q)));
    }
    // each row's parent row: the nearest ancestor that is a row (the spine hangs off RootRotY in the FLVER: the pelvis)
    let row_of = |name: &str| ROWS.iter().position(|r| r.source == name);
    let mut parent = Vec::new();
    for (k, r) in ROWS.iter().enumerate() {
        let (mut at, _, _) = nodes[r.source];
        let mut found = None;
        for _ in 0..64 {
            let p = parents.get(&at).copied().unwrap_or(-1);
            if p < 0 {
                break;
            }
            at = p as usize;
            if let Some(row) = by_index.get(&at).and_then(|n| row_of(n)) {
                found = Some(row);
                break;
            }
        }
        parent.push(if k == 0 { None } else { found.or(Some(0)) });
    }
    // facing +Y, the toes on the floor
    let fwd = {
        let f = (bind[17].0 - bind[16].0) + (bind[21].0 - bind[20].0);
        Vec3::new(f.x, f.y, 0.0).normalize()
    };
    let turn = Quat::from_rotation_arc(fwd, Vec3::Y);
    let floor = bind[17].0.z.min(bind[21].0.z);
    let pelvis = bind[0].0;
    let bind = bind.iter().map(|(p, q)| (turn * (*p - Vec3::new(pelvis.x, pelvis.y, floor)), (turn * *q).normalize())).collect();
    Some(Source { bind, parent, miss: std::cell::Cell::new(0.0) })
}

impl Source {
    fn children(&self, k: usize) -> Vec<usize> {
        let mut out = vec![k];
        let mut i = 0;
        while i < out.len() {
            let at = out[i];
            for (c, p) in self.parent.iter().enumerate() {
                if *p == Some(at) {
                    out.push(c);
                }
            }
            i += 1;
        }
        out
    }

    fn turn(&self, pose: &mut [(Vec3, Quat)], k: usize, about: Vec3, r: Quat) {
        for c in self.children(k) {
            pose[c].0 = about + r * (pose[c].0 - about);
            pose[c].1 = (r * pose[c].1).normalize();
        }
    }

    /// A pose: the pelvis moved by `pelvis` (from its bind place), the torso leaned forward `lean` degrees, each limb's
    /// end solved to its goal (None: as in the bind).
    fn pose(&self, pelvis: Vec3, lean: f32, goals: [Option<Vec3>; 4]) -> Vec<(Vec3, Quat)> {
        let mut p = self.bind.clone();
        for v in p.iter_mut() {
            v.0 += pelvis;
        }
        if lean != 0.0 {
            let spine = ROWS.iter().position(|r| r.source == "Spine").unwrap();
            let at = p[spine].0;
            self.turn(&mut p, spine, at, Quat::from_rotation_x(-lean.to_radians())); // (forward: the head toward +Y)
        }
        // (the knees bent a little forward first: the solve below keeps the side a joint is bent to, and the bind's legs are
        // straight; a sit or a squat then raises the knees as a body does, not through the floor)
        for (j, leg) in [propfit::rows::LEG_L, propfit::rows::LEG_R].iter().enumerate() {
            let at = p[leg[1]].0;
            self.turn(&mut p, leg[1], at, Quat::from_rotation_x((-25.0f32).to_radians()));
            // (a foot far in front of its hip, as sitting: the thigh raised first, the knee comes up)
            if let Some(goal) = goals[2 + j] {
                if goal.y - p[leg[0]].0.y > 20.0 {
                    let hip = p[leg[0]].0;
                    self.turn(&mut p, leg[0], hip, Quat::from_rotation_x(70.0f32.to_radians()));
                }
            }
        }
        for (k, goal) in goals.iter().enumerate() {
            let Some(goal) = goal else { continue };
            let [a, b, c] = propfit::rows::LIMBS[k];
            for _ in 0..2 {
                let Some((q1, q2)) = crate::limbs::two_bone(p[a].0, p[b].0, p[c].0, *goal) else { break };
                let pa = p[a].0;
                self.turn(&mut p, a, pa, q1);
                let pb = p[b].0;
                self.turn(&mut p, b, pb, q2);
            }
            self.miss.set(self.miss.get().max((p[c].0 - *goal).length()));
        }
        p
    }

    fn take_miss(&self) -> f32 {
        self.miss.replace(0.0)
    }
}

/// A target character.
struct Target {
    name: &'static str,
    skeleton: Skeleton,
    base: Vec<Trs>,
    profile: Profile,
    colour: &'static str,
    /// (party) his own animated member (his walk)
    #[cfg(feature = "party")]
    member: Option<kh_partyhost::poser::PosedMember>,
}

fn read_targets() -> Option<Vec<Target>> {
    let root = project().join("build/sora_pack");
    let animator = kh_anim::Animator::new(kh_anim::Pack::load(&root).ok()?, kh_anim::Settings::default()).ok()?;
    let sora = animator.skeleton().clone();
    let base = animator.sample(kh_core::AnimId::IdleN.sequences()[0], 0.0);
    let base = if base.len() == sora.len() { base } else { sora.reference_pose() };
    let mut out = vec![Target { name: "Sora", skeleton: sora, base, profile: Profile::SORA, colour: "#d23", #[cfg(feature = "party")] member: None }];
    #[cfg(feature = "party")]
    {
        let party = root.join("party");
        let read = |p: &Path| -> Option<serde_json::Value> { serde_json::from_str(std::fs::read_to_string(p).ok()?.trim_start_matches('\u{feff}')).ok() };
        let index = read(&party.join("party.json"))?;
        for (id, npc, name, profile, colour) in [(kh_partyhost::poser::DONALD, "n_ex001", "Donald", Profile::DONALD, "#23c"), (kh_partyhost::poser::GOOFY, "n_ex002", "Goofy", Profile::GOOFY, "#283")] {
            let dir = party.join(npc);
            let text = std::fs::read_to_string(dir.join("manifest.json")).ok()?;
            let skeleton = Skeleton::from_manifest(&kh_anim::json::Json::parse(text.trim_start_matches('\u{feff}')).ok()?).ok()?;
            let manifest = read(&dir.join("manifest.json"))?;
            let entry = index["members"].as_array().into_iter().flatten().find(|m| m["member"].as_u64() == Some(id as u64)).cloned().unwrap_or(serde_json::Value::Null);
            let animator = kh_anim::Animator::new(kh_anim::Pack::load(&dir).ok()?, kh_anim::Settings { leg_ik: false, ..kh_anim::Settings::default() }).ok()?;
            let mut member = kh_partyhost::poser::PosedMember::new(id, animator, &manifest, &entry).ok();
            let base = match member.as_mut().map(|m| (m.debug_pose("IdleN", 0.0), m)) {
                Some((Ok(d), m)) => {
                    m.debug = Some(d);
                    let _ = m.step_debug(3, DT, &kh_anim::NoIkWorld);
                    m.debug = None;
                    m.current.local.clone()
                }
                _ => skeleton.reference_pose(),
            };
            let base = if base.len() == skeleton.len() { base } else { skeleton.reference_pose() };
            out.push(Target { name, skeleton, base, profile, colour, member });
        }
    }
    Some(out)
}

/// What an action touches (world, his floor z = 0, facing +Y).
#[derive(Clone, Copy, Debug)]
enum Contact {
    /// A plane y = this (a door, the fog), the hands on it.
    Plane { y: f32 },
    /// Points the hands hold (a handle, a lever, a lid, an item).
    Points([Option<Vec3>; 2]),
    /// Nothing (the grace).
    None,
}

struct Action {
    name: &'static str,
    kind: Kind,
    contact: Contact,
    /// The largest miss of the source's own limbs on their goals (cm): the pose is a fair one when small.
    src_miss: f32,
    /// The source's frames (root place of each).
    frames: Vec<(Vec3, Vec<(Vec3, Quat)>)>,
}

fn actions(src: &Source) -> Vec<Action> {
    // (the Tarnished's reach: shoulders 139 cm, arms 51; hips 97, legs 87: every goal below is within it)
    let mut out = Vec::new();
    out.push(Action { name: "door_double_push", kind: Kind::Surface, contact: Contact::Plane { y: 55.0 }, frames: vec![(Vec3::ZERO, src.pose(Vec3::new(0.0, 12.0, -6.0), 18.0, [Some(Vec3::new(25.0, 55.0, 125.0)), Some(Vec3::new(-25.0, 55.0, 125.0)), Some(Vec3::new(12.0, 30.0, 11.0)), Some(Vec3::new(-12.0, -25.0, 11.0))]))], src_miss: 0.0 });
    out.last_mut().unwrap().src_miss = src.take_miss();
    out.push(Action { name: "door_single_pull", kind: Kind::Surface, contact: Contact::Points([Some(Vec3::new(14.0, 38.0, 105.0)), None]), frames: vec![(Vec3::ZERO, src.pose(Vec3::new(0.0, -5.0, -4.0), -6.0, [Some(Vec3::new(14.0, 38.0, 105.0)), None, Some(Vec3::new(12.0, 10.0, 11.0)), Some(Vec3::new(-12.0, -20.0, 11.0))]))], src_miss: 0.0 });
    out.last_mut().unwrap().src_miss = src.take_miss();
    out.push(Action { name: "lever", kind: Kind::Surface, contact: Contact::Points([Some(Vec3::new(10.0, 40.0, 100.0)), Some(Vec3::new(-10.0, 40.0, 100.0))]), frames: vec![(Vec3::ZERO, src.pose(Vec3::new(0.0, 4.0, -8.0), 10.0, [Some(Vec3::new(10.0, 40.0, 100.0)), Some(Vec3::new(-10.0, 40.0, 100.0)), Some(Vec3::new(12.0, 12.0, 11.0)), Some(Vec3::new(-12.0, -15.0, 11.0))]))], src_miss: 0.0 });
    out.last_mut().unwrap().src_miss = src.take_miss();
    out.push(Action { name: "chest_lid", kind: Kind::Surface, contact: Contact::Points([Some(Vec3::new(20.0, 50.0, 70.0)), Some(Vec3::new(-20.0, 50.0, 70.0))]), frames: vec![(Vec3::ZERO, src.pose(Vec3::new(0.0, 0.0, -25.0), 45.0, [Some(Vec3::new(20.0, 50.0, 70.0)), Some(Vec3::new(-20.0, 50.0, 70.0)), Some(Vec3::new(14.0, 5.0, 11.0)), Some(Vec3::new(-14.0, -10.0, 11.0))]))], src_miss: 0.0 });
    out.last_mut().unwrap().src_miss = src.take_miss();
    out.push(Action { name: "pickup", kind: Kind::Surface, contact: Contact::Points([None, Some(Vec3::new(-8.0, 45.0, 8.0))]), frames: vec![(Vec3::ZERO, src.pose(Vec3::new(0.0, -15.0, -60.0), 80.0, [None, Some(Vec3::new(-8.0, 45.0, 8.0)), Some(Vec3::new(14.0, 25.0, 11.0)), Some(Vec3::new(-14.0, -25.0, 11.0))]))], src_miss: 0.0 });
    out.last_mut().unwrap().src_miss = src.take_miss();
    out.push(Action { name: "grace_sit", kind: Kind::Free, contact: Contact::None, frames: vec![(Vec3::ZERO, src.pose(Vec3::new(0.0, -10.0, -65.0), -5.0, [Some(Vec3::new(22.0, 25.0, 45.0)), Some(Vec3::new(-22.0, 25.0, 45.0)), Some(Vec3::new(20.0, 45.0, 11.0)), Some(Vec3::new(-20.0, 45.0, 11.0))]))], src_miss: 0.0 });
    out.last_mut().unwrap().src_miss = src.take_miss();
    // (ANIMS, the user's fog gate) the push into the mist before the walk: both arms forward and up to part it, standing
    out.push(Action { name: "fog_push", kind: Kind::Surface, contact: Contact::None, frames: vec![(Vec3::ZERO, src.pose(Vec3::new(0.0, 5.0, 0.0), 8.0, [Some(Vec3::new(20.0, 45.0, 155.0)), Some(Vec3::new(-20.0, 45.0, 150.0)), Some(Vec3::new(10.0, 15.0, 11.0)), Some(Vec3::new(-10.0, -15.0, 11.0))]))], src_miss: 0.0 });
    out.last_mut().unwrap().src_miss = src.take_miss();
    // the fog: 1.8 m in 3 s, the Tarnished's strides of 90 cm (each foot planted 0.75 s, swung 0.75 s), a hand ahead
    let mut fog = Vec::new();
    for f in 0..=180 {
        let time = f as f32 * DT;
        let root = Vec3::new(0.0, 60.0 * time, 0.0);
        let foot_at = |phase: f32, x: f32| {
            let cycle = (time + phase) / 1.5;
            let n = cycle.floor();
            let u = cycle - n;
            let y = 22.5 - 60.0 * phase + if u < 0.5 { n * 90.0 } else { n * 90.0 + 90.0 * ((u - 0.5) / 0.5) };
            let lift = if u < 0.5 { 0.0 } else { 15.0 * (std::f32::consts::PI * (u - 0.5) / 0.5).sin() };
            Vec3::new(x, y, 11.0 + lift)
        };
        fog.push((root, src.pose(root + Vec3::new(0.0, 0.0, -3.0), 8.0, [Some(Vec3::new(18.0, root.y + 40.0, 128.0)), None, Some(foot_at(0.0, 10.0)), Some(foot_at(0.75, -10.0))])));
    }
    out.push(Action { name: "fog_walk", kind: Kind::Surface, contact: Contact::None, frames: fog, src_miss: 0.0 });
    out.last_mut().unwrap().src_miss = src.take_miss();
    out
}

/// One character's result of a frame: his joints (world) and the errors.
#[derive(Clone, Debug, Default)]
struct Frame {
    /// The main chains (spine to head, each arm, each leg), world.
    chains: Vec<Vec<Vec3>>,
    /// hands / feet ends (limbs.rs order)
    ends: [Vec3; 4],
    knees: [Vec3; 2],
    shoulders: [Vec3; 2],
    hips: [Vec3; 2],
    head: Vec3,
    pelvis: Vec3,
    chest: Vec3,
}

fn solve(t: &Target, retarget: &(kh_anim::retarget::Retarget, Vec<usize>), sdims: &Dims, tdims: &Dims, fit: Option<&mut Fit>, kind: Kind, root: Vec3, src: &[(Vec3, Quat)]) -> Frame {
    let to_world = Trs { translation: root, rotation: Quat::IDENTITY, scale: Vec3::ONE };
    let rows: Vec<(Vec3, Quat)> = retarget.1.iter().map(|k| src[*k]).collect();
    let mut local = t.base.clone();
    let floor = root.z;
    match fit {
        None => {
            // BEFORE: the rule as it was (sora.rs step_pose / leader.rs PoseOver)
            let pelvis = retarget.0.pelvis_place(src[0].0, Vec3::Z, floor);
            retarget.0.apply(&t.skeleton, &rows, pelvis, &to_world, 1.0, &mut local);
            for k in 0..2 {
                let to = src[propfit::rows::LIMBS[k][2]].0 - root;
                crate::limbs::reach_sora(&t.skeleton, &mut local, k, to, 1.0);
            }
        }
        Some(fit) => {
            let plan = fit.plan(src, sdims, tdims, retarget.0.ratio, &t.profile, root, floor, kind, DT);
            if std::env::var("KH_PROOF_PLAN").is_ok_and(|v| v == t.name) { eprintln!("plan {}: pelvis {:?} shift {:?} limbs {:?} | src hips {:?} {:?}", t.name, plan.pelvis, plan.shift, plan.limbs, src[14].0, src[18].0); }
            let own = local.clone();
            propfit::apply(&retarget.0, &t.skeleton, &rows, &plan, &to_world, 1.0, 1.0, &mut local);
            // (as in the game: a plan the sanity refuses, or a drawn pose past the limits, shows his own pose)
            {
                let bad = if plan.sane { propfit::limits(&t.skeleton, &kh_anim::pose::component_space(&t.skeleton, &own), &kh_anim::pose::component_space(&t.skeleton, &local)).err() } else { Some("the plan's sanity") };
                if let Some(why) = bad {
                    FALLBACK.with(|f| f.borrow_mut().push(format!("{}: {why}", t.name)));
                    local = own;
                }
            }
        }
    }
    let c = kh_anim::pose::component_space(&t.skeleton, &local);
    let at = |n: &str| t.skeleton.index_of(n).map(|k| c[k].translation + root).unwrap_or(root);
    let names = crate::limbs::LIMBS;
    Frame {
        chains: [
            vec!["center", "hara1", "hara2", "mune", "kubi", "atama"],
            vec!["mune", "L_kata", "L_ude1", "L_ude2", "L_te"],
            vec!["mune", "R_kata", "R_ude1", "R_ude2", "R_te"],
            vec!["center", "L_momo", "L_sune", "L_ashi1", "L_ashi2"],
            vec!["center", "R_momo", "R_sune", "R_ashi1", "R_ashi2"],
        ]
        .iter()
        .map(|ch| ch.iter().filter_map(|n| t.skeleton.index_of(n).or_else(|| (*n == "kubi").then(|| t.skeleton.index_of("kubi1")).flatten()).map(|k| c[k].translation + root)).collect())
        .collect(),
        ends: [at(names[0][2]), at(names[1][2]), at(names[2][2]), at(names[3][2])],
        knees: [at("L_sune"), at("R_sune")],
        shoulders: [at("L_ude1"), at("R_ude1")],
        hips: [at("L_momo"), at("R_momo")],
        head: at("atama"),
        pelvis: at("center"),
        chest: at("mune"),
    }
}

/// The errors of a frame against the action's contacts (cm): (hands, feet, penetration).
fn errors(a: &Action, f: &Frame, tdims: &Dims, src: &[(Vec3, Quat)], root: Vec3, swinging: [bool; 2], still: [bool; 4]) -> (f32, f32, f32) {
    let floor = root.z;
    let mut hand = 0.0f32;
    let mut feet = 0.0f32;
    let mut pen = 0.0f32;
    let _ = still;
    match a.contact {
        Contact::Plane { y } => {
            for k in 0..2 {
                hand = hand.max((f.ends[k].y - y).abs());
            }
            for p in [f.knees[0], f.knees[1], f.pelvis, f.chest, f.ends[2], f.ends[3]] {
                pen = pen.max(p.y - y);
            }
        }
        Contact::Points(pts) => {
            for k in 0..2 {
                if let Some(p) = pts[k] {
                    hand = hand.max((f.ends[k] - (p + Vec3::new(root.x, root.y, 0.0))).length());
                }
            }
            // (a handle, a lever, a lid stand in something; an item on the ground does not)
            let plane = pts.iter().flatten().filter(|p| p.z > 50.0).map(|p| p.y).fold(f32::MAX, f32::min);
            for p in [f.knees[0], f.knees[1], f.pelvis, f.chest, f.ends[2], f.ends[3]] {
                pen = pen.max(p.y - plane);
            }
        }
        Contact::None => {}
    }
    // (nothing of his under the ground: the knees, the pelvis)
    for p in [f.knees[0], f.knees[1], f.pelvis] {
        if -p.z > 0.5 && std::env::var("KH_PROOF_DEBUG").is_ok() {
            eprintln!("{}: a knee / the pelvis {:.1} cm under the ground", a.name, -p.z);
        }
        pen = pen.max(-p.z);
    }
    for k in 2..4 {
        // a planted source foot: his sole on the floor
        let sf = src[propfit::rows::LIMBS[k][2]].0;
        if sf.z - floor < 12.0 && !swinging[k - 2] {
            feet = feet.max((f.ends[k].z - tdims.ankle_h - floor).abs());
        }
    }
    (hand, feet, pen.max(0.0))
}

/// The contact a hand should be on (the nearest point of it to the hand as he holds it): the plane, the point.
fn contact_of(a: &Action, k: usize, f: &Frame, root: Vec3) -> Option<Vec3> {
    let h = f.ends[k];
    match a.contact {
        Contact::Plane { y } => Some(Vec3::new(h.x, y, h.z)),
        Contact::Points(p) => p[k].map(|p| p + Vec3::new(root.x, root.y, 0.0)),
        Contact::None => None,
    }
}

fn svg_figure(svg: &mut String, f: &Frame, dx: f32, colour: &str, dash: bool, scale: f32, base_y: f32) {
    // side view: world y -> right, z -> up
    let x = |p: Vec3| dx + p.y * scale;
    let y = |p: Vec3| base_y - p.z * scale;
    for ch in &f.chains {
        let pts: Vec<String> = ch.iter().map(|p| format!("{:.1},{:.1}", x(*p), y(*p))).collect();
        let _ = write!(svg, r##"<polyline points="{}" fill="none" stroke="{colour}" stroke-width="{}" stroke-linejoin="round" {}/>"##, pts.join(" "), if dash { 1.4 } else { 2.4 }, if dash { r##"stroke-dasharray="4 3" opacity="0.7""## } else { "" });
    }
    for e in f.ends {
        let _ = write!(svg, r##"<circle cx="{:.1}" cy="{:.1}" r="2.2" fill="{colour}" {}/>"##, x(e), y(e), if dash { r##"opacity="0.5""## } else { "" });
    }
}

fn svg_contacts(svg: &mut String, a: &Action, dx: f32, scale: f32, base_y: f32, root: Vec3) {
    let x = |yy: f32| dx + yy * scale;
    let y = |z: f32| base_y - z * scale;
    let _ = write!(svg, r##"<line x1="{:.1}" y1="{:.1}" x2="{:.1}" y2="{:.1}" stroke="#888" stroke-width="1"/>"##, x(root.y - 80.0), y(0.0), x(root.y + 120.0), y(0.0));
    match a.contact {
        Contact::Plane { y: py } => {
            let _ = write!(svg, r##"<rect x="{:.1}" y="{:.1}" width="{:.1}" height="{:.1}" fill="#cba" opacity="0.5"/>"##, x(py), y(250.0), 6.0 * scale, 250.0 * scale);
        }
        Contact::Points(pts) => {
            for p in pts.iter().flatten() {
                let _ = write!(svg, r##"<circle cx="{:.1}" cy="{:.1}" r="4" fill="none" stroke="#c80" stroke-width="2"/>"##, x(p.y + root.y), y(p.z));
            }
        }
        Contact::None => {}
    }
}

#[test]
fn propfit_sheet() {
    let (Some(src), Some(targets)) = (read_source(), read_targets()) else {
        eprintln!("propfit_sheet: no figskel_p.txt or no packs: skipped");
        return;
    };
    let out_dir = std::env::var("KH_PROOF_DIR").map(PathBuf::from).unwrap_or_else(|_| project().join("build/sora_tmp/party/ANIMS/proof"));
    let _ = std::fs::create_dir_all(&out_dir);
    let sdims = Dims::of_binds(&src.bind).expect("the Tarnished's measures");
    let mut report = String::new();
    let _ = writeln!(report, "Tarnished: pelvis {:.1} cm, ankle {:.1}, arms {:.1} / {:.1}, legs {:.1} / {:.1}", sdims.pelvis_h, sdims.ankle_h, sdims.limb[0], sdims.limb[1], sdims.limb[2], sdims.limb[3]);
    let mut rigs = Vec::new();
    for t in &targets {
        let r = crate::figure::retarget_from_binds(&src.bind, &t.skeleton).expect("retarget");
        let d = Dims::of_skeleton(&t.skeleton).expect("measures");
        let _ = writeln!(report, "{}: leg ratio {:.3}, pelvis {:.1} cm, ankle {:.1}, arms {:.1} / {:.1}, legs {:.1} / {:.1}, style keep {:.2}, depth {:.0}", t.name, r.0.ratio, d.pelvis_h, d.ankle_h, d.limb[0], d.limb[1], d.limb[2], d.limb[3], t.profile.style_keep, t.profile.depth);
        rigs.push((r, d));
    }
    let _ = writeln!(report, "
{:<18} {:<7} | {:>40} | {:>18} | {:>16}", "action", "who", "hand to contact cm (before -> after)", "sole to floor/rung", "through the plane");
    let mut rows_html = String::new();
    let mut summary: Vec<(String, String, [f32; 6], f32, f32)> = Vec::new();
    let mut fallbacks: Vec<(String, String, usize)> = Vec::new();
    for a in actions(&src) {
        let _ = writeln!(report, "{:<18} (the Tarnished's own pose misses its goals by {:.1} cm at most)", a.name, a.src_miss);
        let scale = 1.1;
        let col_w = 330.0;
        let (w, h) = (col_w * (targets.len() as f32 + 1.0), 360.0);
        let mut svg = format!(r##"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}" font-family="sans-serif" font-size="12"><rect width="100%" height="100%" fill="#fff"/>"##);
        // the frame drawn: the middle of a sequence
        let shown = a.frames.len() / 2;
        // the Tarnished
        {
            let (root, s) = &a.frames[shown];
            let dx = 90.0 - root.y * scale;
            let base_y = h - 30.0 + root.z.min(0.0);
            let base_y = base_y + (root.z * scale).min(220.0);
            svg_contacts(&mut svg, &a, dx, scale, base_y, *root);
            for k in 0..ROWS.len() {
                if let Some(p) = src.parent[k] {
                    let (q, v) = (s[p].0, s[k].0);
                    let _ = write!(svg, r##"<line x1="{:.1}" y1="{:.1}" x2="{:.1}" y2="{:.1}" stroke="#666" stroke-width="1.8"/>"##, dx + q.y * scale, base_y - q.z * scale, dx + v.y * scale, base_y - v.z * scale);
                }
            }
            let _ = write!(svg, r##"<text x="{:.0}" y="18">Tarnished (source)</text>"##, 10.0);
        }
        for (ti, t) in targets.iter().enumerate() {
            let (rig, tdims) = &rigs[ti];
            let mut fit = Fit::default();
            let mut worst = [0.0f32; 6];
            let mut slide_b = 0.0f32;
            let mut slide_a = 0.0f32;
            let mut short = 0.0f32;
            let mut was_swinging = [false; 2];
            let mut prev_b: Option<Frame> = None;
            let mut prev_a: Option<Frame> = None;
            let mut shown_frames = (Frame::default(), Frame::default());
            for (fi, (root, s)) in a.frames.iter().enumerate() {
                let before = solve(t, rig, &sdims, tdims, None, a.kind, *root, s);
                let after = solve(t, rig, &sdims, tdims, Some(&mut fit), a.kind, *root, s);
                let swinging = [fit.swinging(0), fit.swinging(1)];
                let still: [bool; 4] = std::array::from_fn(|k| fi == 0 && a.frames.len() == 1 || fi > 0 && { let e = propfit::rows::LIMBS[k][2]; (s[e].0 - a.frames[fi - 1].1[e].0).length() < 0.3 });
                let (hb, fb, pb) = errors(&a, &before, tdims, s, *root, [false, false], still);
                let (ha, fa, pa) = errors(&a, &after, tdims, s, *root, swinging, still);
                if std::env::var("KH_PROOF_DEBUG").is_ok() && a.name == "fog_walk" && t.name == "Donald" && fi % 3 == 0 { eprintln!("fog D f{fi} root {:.1} feet {:?} sw {:?} fa {fa:.1} hipR {:?} d {:.1} leg {:.1}", root.y, [after.ends[2], after.ends[3]], swinging, after.hips[1], (after.hips[1] - after.ends[3]).length(), tdims.limb[3]); }
                if std::env::var("KH_PROOF_DEBUG").is_ok() && fi == 0 && a.name == "grace_sit" { eprintln!("sit {}: src knees {:?} {:?} his knees {:?} pelvis {:?}", t.name, s[15].0, s[19].0, after.knees, after.pelvis); }
                if std::env::var("KH_PROOF_DEBUG").is_ok() && fi == 0 { eprintln!("{} {}: after ends {:?} src ends {:?}", a.name, t.name, after.ends, propfit::rows::LIMBS.map(|l| s[l[2]].0)); }
                worst = [worst[0].max(hb), worst[1].max(ha), worst[2].max(fb), worst[3].max(fa), worst[4].max(pb), worst[5].max(pa)];
                // what his arms cannot reach at all from where he stands (the share of the hand's miss that is no fault)
                for k in 0..2 {
                    if let Some(c) = contact_of(&a, k, &after, *root) {
                        short = short.max(((c - after.shoulders[k]).length() - tdims.limb[k]).max(0.0));
                    }
                }
                // sliding: a foot that should stand (the Tarnished's is planted; after: not in one of his own steps) moves
                for j in 0..2 {
                    let k = propfit::rows::LIMBS[2 + j][2];
                    let planted = fi > 0 && (s[k].0 - a.frames[fi - 1].1[k].0).length() < 0.3 && s[k].0.z < 12.0 + root.z;
                    if let (true, Some(pb), Some(pa)) = (planted, &prev_b, &prev_a) {
                        slide_b = slide_b.max((before.ends[2 + j] - pb.ends[2 + j]).length());
                        if !swinging[j] && !was_swinging[j] {
                            slide_a = slide_a.max((after.ends[2 + j] - pa.ends[2 + j]).length());
                        }
                    }
                }
                was_swinging = swinging;
                if fi == shown {
                    shown_frames = (before.clone(), after.clone());
                }
                prev_b = Some(before);
                prev_a = Some(after);
            }
            let (root, _) = &a.frames[shown];
            let dx = col_w * (ti as f32 + 1.0) + 90.0 - root.y * scale;
            let base_y = h - 30.0 + (root.z * scale).min(220.0);
            svg_contacts(&mut svg, &a, dx, scale, base_y, *root);
            svg_figure(&mut svg, &shown_frames.0, dx, "#999", true, scale, base_y);
            svg_figure(&mut svg, &shown_frames.1, dx, t.colour, false, scale, base_y);
            let _ = write!(svg, r##"<text x="{:.0}" y="18" fill="{}">{} (after; grey dashed = before)</text>"##, col_w * (ti as f32 + 1.0) + 10.0, t.colour, t.name);
            let _ = write!(svg, r##"<text x="{:.0}" y="34" fill="#333" font-size="11">hands off contact {:.1} -> {:.1} cm (out of his reach {:.1})</text>"##, col_w * (ti as f32 + 1.0) + 10.0, worst[0], worst[1], short);
            let _ = write!(svg, r##"<text x="{:.0}" y="48" fill="#333" font-size="11">soles off floor {:.1} -> {:.1}, through {:.1} -> {:.1}{}</text>"##, col_w * (ti as f32 + 1.0) + 10.0, worst[2], worst[3], worst[4], worst[5], if a.frames.len() > 1 { format!(", slide {slide_b:.1} -> {slide_a:.1}") } else { String::new() });
            let _ = writeln!(report, "{:<18} {:<7} | {:>9.1} -> {:>6.1} (out of reach {:>5.1}) | {:>8.1} -> {:>6.1} | {:>7.1} -> {:>6.1}{}", a.name, t.name, worst[0], worst[1], short, worst[2], worst[3], worst[4], worst[5], if a.frames.len() > 1 { format!("   foot slide per frame {slide_b:.2} -> {slide_a:.2} cm") } else { String::new() });
            let fb = FALLBACK.with(|f| std::mem::take(&mut *f.borrow_mut()));
            if !fb.is_empty() {
                let _ = writeln!(report, "{:<18} {:<7} | HIS OWN POSE in {} of {} frame(s) (the guards: {})", a.name, t.name, fb.len(), a.frames.len(), fb[0]);
            }
            fallbacks.push((a.name.to_string(), t.name.to_string(), fb.len()));
            // the drawn pose: his hands never above his head
            for k in 0..2 {
                let over = (0.6 * (shown_frames.1.head - shown_frames.1.chest).length()).max(8.0);
                assert!(shown_frames.1.ends[k].z <= shown_frames.1.head.z + over, "{} {}: a hand above his head", a.name, t.name);
            }
            summary.push((a.name.to_string(), t.name.to_string(), worst, slide_b, slide_a));
        }
        svg.push_str("</svg>");
        let file = format!("{}.svg", a.name);
        std::fs::write(out_dir.join(&file), &svg).unwrap();
        let _ = write!(rows_html, r##"<h2>{}</h2><img src="{file}" style="max-width:100%">"##, a.name);
    }
    // (ANIMS, the user's fog gate) THE WALK THROUGH ON THEIR OWN WALK: Donald and Goofy walked 1.8 m in 3 s by the body
    // (the fog's 60060), each drawn with his own `WalkRunN` as the game would draw him: upright (the limits against his
    // idle every frame), his planted feet still on the ground
    #[cfg(feature = "party")]
    for t in targets.iter() {
        let Some(m) = t.member.as_ref() else { continue };
        let mut worst_slide = 0.0f32;
        let mut broken = 0;
        let mut prev: Option<[Vec3; 2]> = None;
        let foot_min = |c: &[Trs], n: &str| t.skeleton.index_of(n).map(|k| c[k].translation);
        let mut lows = [f32::MAX; 2];
        let mut frames = Vec::new();
        for f in 0..=180 {
            let distance = 60.0 * f as f32 * DT;
            let Some(walk) = crate::party::leader::walk_local(m, distance, 60.0) else { panic!("{}: no walk", t.name) };
            let mut local = t.base.clone();
            crate::party::leader::blend_body(&t.skeleton, &walk, 1.0, &mut local);
            let c = kh_anim::pose::component_space(&t.skeleton, &local);
            if let Err(why) = propfit::limits(&t.skeleton, &kh_anim::pose::component_space(&t.skeleton, &t.base), &c) {
                if broken == 0 {
                    eprintln!("{} walk frame {f}: {why}", t.name);
                }
                broken += 1;
            }
            let root = Vec3::new(0.0, distance, 0.0);
            let feet = [foot_min(&c, "L_ashi1").unwrap() + root, foot_min(&c, "R_ashi1").unwrap() + root];
            for j in 0..2 {
                lows[j] = lows[j].min(feet[j].z);
            }
            frames.push(feet);
            let _ = prev.replace(feet);
        }
        // a foot within 1 cm of its lowest is planted: its slide along the ground between two planted frames
        for w in frames.windows(2) {
            for j in 0..2 {
                if w[0][j].z < lows[j] + 1.0 && w[1][j].z < lows[j] + 1.0 {
                    worst_slide = worst_slide.max((w[1][j] - w[0][j]).truncate().length());
                }
            }
        }
        let _ = writeln!(report, "{:<18} {:<7} | his own walk at 60 cm/s: limits broken in {broken} of 181 frames, planted foot slide {worst_slide:.2} cm a frame", "fog_walk_own", t.name);
        assert_eq!(broken, 0, "{}: his own walk breaks the limits", t.name);
        assert!(worst_slide < 0.8, "{}: his own walk slides {worst_slide} cm a frame", t.name);
    }
    std::fs::write(out_dir.join("report.txt"), &report).unwrap();
    // THE FOG PUSH (the user's screen: hands up by his head, the head tucked): every character's borrowed push is drawn
    // without the guards having to step in (arms at his height, his head his own)
    for (action, who, n) in &fallbacks {
        if action == "fog_push" {
            assert_eq!(*n, 0, "{who}: the fog push needed the fallback ({})", report.lines().find(|l| l.starts_with("fog_push") && l.contains(who.as_str()) && l.contains("HIS OWN POSE")).unwrap_or(""));
        }
    }
    std::fs::write(out_dir.join("report.txt"), &report).unwrap();
    let html = format!(r##"<!doctype html><meta charset="utf-8"><title>ANIMS proof sheet</title><body style="font-family:sans-serif;background:#fff;color:#111;margin:16px"><h1>Borrowed Elden Ring actions at each character's proportions</h1><pre>{report}</pre>{rows_html}</body>"##);
    std::fs::write(out_dir.join("index.html"), html).unwrap();
    eprintln!("{report}\nproof sheets in {}", out_dir.display());
    // THE CLAIMS (after): hands on their contacts where his reach allows, soles on the floor, nothing through the door, no
    // planted foot sliding
    for (action, who, w, _, slide_a) in &summary {
        assert!(w[5] <= 1.0, "{action} {who}: {:.1} cm through the contact plane", w[5]);
        assert!(*slide_a < 0.5 || who == "Sora", "{action} {who}: a planted foot slides {slide_a:.2} cm a frame");
    }
}
