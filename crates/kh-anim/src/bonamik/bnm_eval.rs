//! `tools/lib/bonamik/bnm_eval.js`: the driver. Links the transcriptions (cone, chain, coll, kdi,
//! reset, wind, hfps, core) into: core setup (0x143b07550), CoreUpdate (0x143b0efc0) and Step
//! (0x143b0cb60). The JS's `create` (owner object, gravity, random stream, runner) is
//! [`super::Bonamik::new`].

use crate::json::Json;

use super::chain;
use super::coll::{self, LinkCollisionDesc, PlaneDesc};
use super::cone;
use super::core as bcore;
use super::hfps;
use super::kdi;
use super::m::{cross, mul, sqrt, Q4, V3};
use super::records::{Anchor, Cfg, Core, Owner, Particle, Scene, Stats};
use super::reset;
use super::wind::{self, Rng};

const LINKTYPE: [(&str, i32); 7] = [("Lateral", 0), ("ParentChild", 1), ("Bending", 2), ("PinConstraint", 3), ("KdiConstraint", 4), ("ReactionTransfer", 5), ("InverseFreeConstraint", 6)];
const SHAPE: [(&str, i32); 8] = [("None", 0), ("Sphere", 1), ("Capsule", 2), ("Inverse_Sphere", 3), ("Inverse_Capsule", 4), ("Plane", 6), ("Box", 8), ("Self_Collision", 9)];
const ADDI: [(&str, i32); 5] = [("None", 0), ("Sphere", 1), ("Spring", 2), ("GroundAttach", 3), ("SecondOrder_Sphere", 4)];
const FAST_MOTION: [(&str, i32); 3] = [("NONE", 0), ("LOCAL_FORCE", 1), ("LSS", 2)];

/// `enumOf`: a number is itself; else the exact key; else the first key the text ends with.
fn enum_of(table: &[(&str, i32)], v: &Json) -> Result<i32, String> {
    if let Some(n) = v.as_f64() {
        return Ok(n as i32);
    }
    let text = v.as_str().ok_or_else(|| format!("enum {v:?}"))?;
    if let Some((_, n)) = table.iter().find(|(k, _)| *k == text) {
        return Ok(*n);
    }
    table.iter().find(|(k, _)| text.ends_with(k)).map(|(_, n)| *n).ok_or_else(|| format!("enum {text}"))
}

fn num(j: &Json, key: &str) -> Result<f64, String> {
    j.get(key).as_f64().ok_or_else(|| format!("bonamik.json: number '{key}' missing"))
}

fn fl(j: &Json, key: &str) -> Result<f32, String> {
    num(j, key).map(|n| n as f32)
}

fn int(j: &Json, key: &str) -> Result<i32, String> {
    num(j, key).map(|n| n as i32)
}

/// A batch number, kept as the JS has it (the asset stores 0xFFFFFFFF as 4294967295).
fn int64(j: &Json, key: &str) -> Result<i64, String> {
    num(j, key).map(|n| n as i64)
}

/// JS truthiness of a flag.
fn flag(j: &Json, key: &str) -> u8 {
    match j.get(key) {
        Json::Bool(b) => *b as u8,
        Json::Number(n) => (*n != 0.0) as u8,
        Json::String(s) => !s.is_empty() as u8,
        Json::Null => 0,
        _ => 1,
    }
}

fn v3(j: &Json, key: &str) -> Result<V3, String> {
    let f = j.get(key).floats();
    if f.len() < 3 {
        return Err(format!("bonamik.json: vector '{key}' missing"));
    }
    Ok([f[0], f[1], f[2]])
}

fn q4(j: &Json, key: &str) -> Result<Q4, String> {
    let f = j.get(key).floats();
    if f.len() < 4 {
        return Err(format!("bonamik.json: quaternion '{key}' missing"));
    }
    Ok([f[0], f[1], f[2], f[3]])
}

/// LinkDesc in the exe's layout (SQEX_BonamikLinkDesc), from a plain row of the asset.
#[derive(Clone, Copy, Debug)]
pub struct LinkDesc {
    /// m_ParentId, m_ChildId
    pub f20: i32,
    pub f24: i32,
    /// m_LinkType
    pub f28: i32,
    /// m_IsSelfCollision, m_EnableJointTwist, cone, m_EnableConeAnimation
    pub f29: u8,
    pub f2a: u8,
    pub f2b: u8,
    pub f2c: u8,
    /// m_LinkStr, m_LinkParentStr, m_ConeInnerStr, m_ConeOuterStr, m_ConeParentStr
    pub f30: f32,
    pub f34: f32,
    pub f38: f32,
    pub f3c: f32,
    pub f40: f32,
    /// m_LimitY, m_LimitZ, m_ConeOffsetY, m_ConeOffsetZ, m_LimitOffsetY, m_LimitOffsetZ (degrees)
    pub f44: f32,
    pub f48: f32,
    pub f4c: f32,
    pub f50: f32,
    pub f54: f32,
    pub f58: f32,
    /// m_IsCollisionReceiver, m_IsCollisionRepulsor, long-range attachment
    pub f5c: u8,
    pub f5d: u8,
    pub f5e: u8,
    pub f60: f32,
    pub f64: f32,
    /// m_LinkBatchNo, m_ConeBatchNo, m_CollisionBatchNo
    pub f68: i64,
    pub f6c: i64,
    pub f70: i64,
    /// m_PinRadius, m_PinStr, m_KdiTrans, m_KdiRot, m_ConcaveRatio
    pub f74: f32,
    pub f78: f32,
    pub f7c: f32,
    pub f80: f32,
    pub f84: f32,
}

pub fn link_desc(l: &Json) -> Result<LinkDesc, String> {
    Ok(LinkDesc {
        f20: int(l, "parent")?,
        f24: int(l, "child")?,
        f28: enum_of(&LINKTYPE, l.get("type"))?,
        f29: flag(l, "selfCollision"),
        f2a: flag(l, "jointTwist"),
        f2b: flag(l, "cone"),
        f2c: flag(l, "coneAnimation"),
        f30: fl(l, "linkStr")?,
        f34: fl(l, "linkParentStr")?,
        f38: fl(l, "coneInnerStr")?,
        f3c: fl(l, "coneOuterStr")?,
        f40: fl(l, "coneParentStr")?,
        f44: fl(l, "limitYdeg")?,
        f48: fl(l, "limitZdeg")?,
        f4c: fl(l, "coneOffsetYdeg")?,
        f50: fl(l, "coneOffsetZdeg")?,
        f54: fl(l, "limitOffsetYdeg")?,
        f58: fl(l, "limitOffsetZdeg")?,
        f5c: flag(l, "collisionReceiver"),
        f5d: flag(l, "collisionRepulsor"),
        f5e: flag(l, "lra"),
        f60: fl(l, "lraStr")?,
        f64: fl(l, "lraDamping")?,
        f68: int64(l, "linkBatchNo")?,
        f6c: int64(l, "coneBatchNo")?,
        f70: int64(l, "collisionBatchNo")?,
        f74: fl(l, "pinRadius")?,
        f78: fl(l, "pinStr")?,
        f7c: fl(l, "kdiTrans")?,
        f80: fl(l, "kdiRot")?,
        f84: fl(l, "concaveRatio")?,
    })
}

fn col_ids(j: &Json) -> [i32; 8] {
    let mut out = [-1; 8];
    for (k, v) in j.items().iter().take(8).enumerate() {
        out[k] = v.as_f64().map_or(-1, |n| n as i32);
    }
    out
}

/// Core setup: 0x143af0550 (defaults) + 0x143b07550 (from the asset). `owner.bones` =
/// `owner.ref_bones` = the component-space reference pose of the mesh (owner vf50, 0x143b585a0).
/// `teleport_distance`: the JS's `opts.teleportDistance` (default: the asset user data's).
pub fn setup_core(data: &Json, owner: Owner, cfg: &Cfg, teleport_distance: Option<f32>) -> Result<Core, String> {
    let g = |v: f32| vec![v; 48];
    let ident: Q4 = [0.0, 0.0, 0.0, 1.0];
    let one: V3 = [1.0, 1.0, 1.0];
    let zero: V3 = [0.0, 0.0, 0.0];
    let n_bones = owner.ref_bones.len();
    let mut core = Core {
        // frame transforms (filled below / by reset.rs)
        f0: ident,
        f10: zero,
        f20: one,
        f30: ident,
        f40: zero,
        f50: one,
        f60: -1,
        f70: ident,
        f80: zero,
        f90: one,
        fa0: ident,
        fb0: zero,
        fc0: one,
        fd0: ident,
        fe0: zero,
        ff0: one,
        f100: ident,
        f110: zero,
        f120: one,
        f130: ident,
        f140: zero,
        f150: one,
        f160: ident,
        f170: zero,
        f180: one,
        f190: ident,
        f1a0: zero,
        f1b0: one,
        // 0x143af0550 defaults
        f1c0: vec![[0.0, -1.0, 0.0]; 48],
        f400: Vec::new(),
        f410: 0,
        f414: vec![0; 48],
        f4d8: vec![String::new(); 48],
        f658: 0,
        f65c: 1,
        f65d: 1,
        f660: 1,
        f664: 0,
        f668: 0.2_f64 as f32,
        f66c: 0.2_f64 as f32,
        f670: 1.0,
        f674: 0,
        f678: 0,
        f67c: 0,
        f680: 0,
        f684: vec![0; 48],
        f744: vec![0; 48],
        f804: vec![2; 48],
        f8c4: vec![0; 48],
        f8f4: vec![1; 48],
        f924: vec![0; 48],
        f954: g(0.0),
        fa14: g(1.0),
        fad4: vec![11; 48],
        fb94: g(1.0),
        fc54: g(1.0),
        fd14: g(1.0),
        fdd4: g(1.0),
        fe94: g(1.0),
        f33e8: 0,
        f33e9: 0,
        f33ea: 0,
        f33ec: 0,
        f33f0: 0,
        f33f4: 1,
        f33f5: 0,
        f33f6: 0,
        f33f8: g(0.0),
        f34b8: g(0.0),
        f3578: g(0.0),
        f3638: g(0.0),
        f36f8: 1.0,
        f36fc: 1.0,
        f3700: 0.0,
        f3704: 0,
        f3708: 1.0,
        f370c: 1.0,
        f3710: 0.0,
        f3714: 0.0,
        f3718: 0,
        f371c: 0.0,
        f3720: 0,
        f3724: zero,
        f3738: 0.0,
        f373c: 0.0,
        f3740: 0.0,
        f3744: 0,
        f3750: 0,
        f3770: 0,
        f3778: Scene::default(),
        f37e8: 0,
        f3800: Vec::new(),
        f3808: 0,
        f3810: Vec::new(),
        f3818: 0,
        f3820: 0,
        f3824: 0,
        f3828: 0,
        f382c: 0.0,
        f3830: 0.0,
        f3834: 0.0,
        f3838: 0.0,
        f383c: 0.0,
        f3840: 0,
        f3848: owner,
    };
    // time step state (head of 0x143b07550)
    core.f3828 = 0;
    let fps = cfg.f5c; // (*0x149DE3B98)->vf[0x280]() if > 0 [?], else ExpectedFrameRate
    let b = bcore::nearest_bucket(1.0 / (1.0 / fps));
    core.f3820 = b;
    core.f3824 = b;
    let st = 1.0 / bcore::FPS_TABLE[b];
    core.f382c = st;
    core.f3830 = st;
    core.f3834 = st;
    core.f3838 = cfg.f60 / cfg.f64;
    core.f383c = (0.6_f64 as f32) / cfg.f60;
    let s = data.get("scene");
    let ver = num(data.get("source"), "versionNo")? as u32;
    core.f658 = ver;
    if ver < 0x960 {
        return Err("version < 2400 not modelled".to_string());
    }
    core.f65d = flag(s, "enableGroundCollision");
    core.f660 = enum_of(&FAST_MOTION, s.get("fastMotionHandlingMode"))?;
    core.f664 = if s.get("jointLinkSimulationMode").as_str().is_some_and(|m| m.contains("BATCH")) { 1 } else { 0 };
    core.f668 = fl(s, "lssCoeffTran")?;
    core.f66c = fl(s, "lssCoeffRot")?;
    core.f674 = num(s, "subStep")? as usize;
    core.f670 = 1.0;
    // scene m_RefUpVec == 1 and version >= 2400
    let up0: V3 = if s.get("refUpVec").as_f64() == Some(1.0) { [0.0, 0.0, 1.0] } else { [0.0, -1.0, 0.0] };
    let td = match teleport_distance {
        Some(td) => td as f64,
        None => data.get("user_data").get("TeleportDistance").as_f64().unwrap_or(f64::NAN),
    };
    core.f371c = if td > 0.0 { (td as f32) * (td as f32) } else { 40000.0 };
    let groups = data.get("groups").items();
    if groups.len() > 48 {
        return Err(format!("bonamik.json: {} groups (the core has 48 slots)", groups.len()));
    }
    core.f410 = groups.len();
    core.f678 = 0;
    core.f67c = 0;
    core.f680 = 1;
    for (i, grp) in groups.iter().enumerate() {
        if grp.get("id").as_usize() != Some(i) {
            return Err("group id != index".to_string());
        }
        core.f1c0[i] = match grp.get("refUpVec").as_f64() {
            Some(v) if v == 0.0 => up0,
            Some(v) if v == 1.0 => [0.0, -1.0, 0.0],
            _ => [0.0, 0.0, 1.0],
        };
        core.f414[i] = i as i32;
        core.f4d8[i] = grp.get("name").as_str().unwrap_or("").to_string();
        core.f684[i] = int(grp, "tagId")?;
        core.f744[i] = int(grp, "collisionTagId")?;
        let enable = flag(grp, "enable") != 0;
        core.f804[i] = if enable { 2 } else { 0 };
        core.f3578[i] = if enable { 0.0 } else { 1.0 };
        core.f3638[i] = core.f3578[i];
        core.f924[i] = flag(grp, "enableGround");
        core.f954[i] = 0.0;
        core.fa14[i] = 1.0;
        let (solver, link) = (num(grp, "solverIter")? as usize, num(grp, "linkIter")? as usize);
        if core.f678 < solver {
            core.f678 = solver;
        }
        if core.f67c < link {
            core.f67c = link;
        }
    }
    core.f3808 = n_bones;
    core.f3818 = n_bones;
    core.f3810 = vec![0.0; n_bones];
    // m_RootName ("Trans"); only used by LSS mode
    let root_name = s.get("rootName").as_str().unwrap_or("");
    core.f60 = core.f3848.names.iter().position(|n| n == root_name).map_or(-1, |i| i as i32);
    let w = core.f3848.world;
    core.f0 = w.q;
    core.f10 = w.t;
    core.f20 = w.s;
    core.f30 = w.q;
    core.f40 = w.t;
    core.f50 = w.s;
    let b0 = core.f3848.ref_bones[0]; // owner.vf88() = 0
    core.f130 = b0.q;
    core.f140 = b0.t;
    core.f150 = b0.s;
    core.f3848.bones = core.f3848.ref_bones.clone();
    reset::fn_143b00530(&mut core);
    // particles (0x143aed230 + constructor 0x143ae5d70) and anchor constraints
    for (i, b) in data.get("bodies").items().iter().enumerate() {
        if b.get("index").as_usize() != Some(i) {
            return Err("bonamik.json: body index != position".to_string());
        }
        let bone_index = b.get("bone_index").as_usize().filter(|k| *k < n_bones).ok_or("bonamik.json: a body names a bone the skeleton does not have")?;
        let bone = core.f3848.ref_bones[bone_index];
        let lt = v3(b, "localTranslation")?;
        let sv = [bone.s[0] * lt[0], bone.s[1] * lt[1], bone.s[2] * lt[2]];
        // T + c2 + w*t + v (order of 143b07f6f..)
        let t = mul(cross(&bone.q, &sv), 2.0);
        let c2 = cross(&bone.q, &t);
        let pos = [((bone.t[0] + c2[0]) + (bone.q[3] * t[0])) + sv[0], ((bone.t[1] + c2[1]) + (bone.q[3] * t[1])) + sv[1], ((bone.t[2] + c2[2]) + (bone.q[3] * t[2])) + sv[2]];
        let mass = fl(b, "mass")?;
        let mut m2 = mass;
        if mass < 0.0 {
            if m2 >= -0.0001_f64 as f32 {
                m2 = -0.0001_f64 as f32;
            }
        } else if m2 <= 0.0001_f64 as f32 {
            m2 = 0.0001_f64 as f32;
        }
        let group = b.get("group").as_usize().filter(|g| *g < core.f410).ok_or("bonamik.json: a body names a group the asset does not have")?;
        let p = Particle {
            f0: pos,
            fc: pos,
            f20: bone.q,
            f30: bone.q,
            f40: zero,
            f4c: zero,
            f58: zero,
            f64: zero,
            f70: zero,
            f7c: zero,
            f88: zero,
            f94: zero,
            fa0: zero,
            fac: zero,
            fb8: 0,
            fbc: 0,
            fbd: 0,
            fc0: bone.q,
            fd0: bone.t,
            fe0: bone.s,
            ff0: bone.q,
            f100: bone.t,
            f110: bone.s,
            f120: bone.q,
            f130: bone.t,
            f140: bone.s,
            f150: v3(b, "localScale")?,
            f160: ident,
            f170: lt,
            f180: q4(b, "localRotation")?,
            f190: None,
            f198: None,
            f1a0: int(b, "parentBody")?,
            f1a4: int(b, "childBody")?,
            f1a8: 0,
            f1b0: [0; 4],
            f1d0: zero,
            f1dc: zero,
            f220: None,
            f228: 0.0,
            f22c: 0.0,
            f230: 0.0,
            f238: None,
            f240: None,
            f248: bone_index,
            f24c: i,
            f250: mass,
            f254: 1.0 / m2,
            f258: fl(b, "inertia")?,
            f25c: fl(b, "radius")?,
            f260: fl(b, "damping")?,
            f264: fl(b, "bounciness")?,
            f268: fl(b, "friction")?,
            f26c: fl(b, "windDrag")?,
            f270: fl(b, "windLift")?,
            f274: 0,
            f275: 0,
            f276: flag(b, "kinematic"),
            f277: flag(b, "collision"),
            f278: flag(b, "multipleCollisionObject"),
            f279: 0,
            f27c: enum_of(&SHAPE, b.get("colShape"))?,
            f280: int64(b, "collisionBatchNo")?,
            f284: fl(b, "concaveRatio")?,
            f288: fl(b, "lfCoeff")?,
            f28c: fl(b, "parkRate")?,
            f290: fl(b, "maxLF")?,
            f294: fl(b, "lfDrag")?,
            f298: fl(b, "lfLift")?,
            f2a0: None,
            f2a8: None,
            f2b8: group,
            f2bc: col_ids(b.get("colId")),
            f2e0: [0; 4],
            f300: [0; 4],
            f310: 0,
            f314: 0.0,
            f318: 0.0,
            f31c: 0,
            f31d: 0,
            f320: fl(b, "lfCoeff")?,
            f324: fl(b, "parkRate")?,
            f328: fl(b, "maxLF")?,
            name: b.get("joint").as_str().unwrap_or("").to_string(),
        };
        core.f3778.f0.push(p);
        let at = enum_of(&ADDI, b.get("addiConstType"))?;
        if at == 3 {
            // core.f37E0 (the ground-attach list); its pass 0x143b104ef..0x143b107fb is not transcribed
            core.f37e8 += 1;
        } else if at != 0 {
            core.f3778.f10.push(Anchor { f0: i, f4: at, f8: fl(b, "addiConstStr")?, fc: pos, f18: zero, f24: fl(b, "addiConstRad")?, f28: pos, f34: zero });
            core.f3778.f0[i].fbd = 1;
        }
    }
    if core.f37e8 > 0 {
        return Err("not transcribed: 0x143b104ef..0x143b107fb (ground attach list)".to_string());
    }
    let n = core.f3778.f0.len();
    for p in core.f3778.f0.iter_mut() {
        if core.f684[p.f2b8] == 11 {
            p.f31d = 1;
        }
        let ok = |i: i32| i == -1 || (i >= 0 && (i as usize) < n);
        if !ok(p.f1a0) || !ok(p.f1a4) {
            return Err("bonamik.json: a body names a parent / child body the asset does not have".to_string());
        }
    }
    if core.f3778.f0.iter().any(|p| p.f31d != 0) {
        return Err("not transcribed: 143b13f63..143b14b2e (ground-attach / tag-11 smoothing variant)".to_string());
    }
    // planes, initial orientations, per-body collision objects
    coll::init_collision_lists(&mut core.f3778);
    let mut planes = Vec::new();
    for pl in data.get("planes").items() {
        planes.push(PlaneDesc {
            m_group_id: pl.get("group").as_usize().filter(|g| *g < core.f410).ok_or("bonamik.json: a plane names a group the asset does not have")?,
            m_particle_a: int(pl, "particleA")?,
            m_particle_b: int(pl, "particleB")?,
            m_particle_c: int(pl, "particleC")?,
            m_thickness: fl(pl, "thickness")?,
            m_offset: fl(pl, "offset")?,
            m_is_collision: flag(pl, "collision") != 0,
            m_collision_batch_no: int64(pl, "collisionBatchNo")?,
            m_col_id: col_ids(pl.get("colId")),
        });
    }
    coll::setup_planes(&mut core, &planes, 0);
    bcore::init_orientations(&mut core);
    coll::setup_body_objects(&mut core);
    // links
    let mut descs = Vec::new();
    for l in data.get("links").items() {
        descs.push(link_desc(l)?);
    }
    for d in &descs {
        let col = LinkCollisionDesc { m_parent_id: d.f20, m_child_id: d.f24, m_is_self_collision: d.f29 != 0, m_is_collision_receiver: d.f5c != 0, m_is_collision_repulsor: d.f5d != 0, m_collision_batch_no: d.f70, m_concave_ratio: d.f84 };
        let in_range = (d.f20 as u32 as usize) < n && (d.f24 as u32 as usize) < n;
        if d.f29 != 0 {
            if !in_range {
                return Err("bonamik.json: a self-collision link names a body the asset does not have".to_string());
            }
            coll::setup_link_collision(&mut core, &col, None, 0);
            continue;
        }
        if d.f5e != 0 {
            if !in_range {
                return Err("bonamik.json: a long-range attachment names a body the asset does not have".to_string());
            }
            let (i1, i2) = (d.f20 as usize, d.f24 as usize);
            let (a, b) = (core.f3778.f0[i1].f0, core.f3778.f0[i2].f0);
            let (dx, dy, dz) = (b[0] - a[0], b[1] - a[1], b[2] - a[2]);
            let p2 = &mut core.f3778.f0[i2];
            p2.f220 = Some(i1);
            p2.f22c = d.f60;
            p2.f230 = d.f64;
            p2.f228 = sqrt(((dx * dx) + (dy * dy)) + (dz * dz));
            continue;
        }
        if d.f28 == 5 {
            return Err("ReactionTransfer link not modelled (none in Sora)".to_string());
        }
        if d.f28 == 3 || d.f28 == 4 {
            continue;
        }
        if !in_range {
            continue;
        }
        let l = chain::fn_143aec770(&mut core.f3778, d, 0);
        if d.f2b != 0 {
            let c = cone::fn_143aec370(&mut core.f3778, d, 0);
            core.f3778.f40[l].f28 = Some(c);
        }
        coll::setup_link_collision(&mut core, &col, Some(l), 0);
    }
    kdi::setup_pin_kdi(&mut core.f3778, &descs, 0);
    bcore::build_roots(&mut core); // roots + hard-length flags (143b0a03d..143b0a324)
    chain::blk_143b0a327(&mut core, n); // joint-twist signs and rest angles
    coll::setup_bone_map(&mut core, n_bones);
    coll::setup_finish_collision(&mut core);
    reset::fn_143b01210(&mut core, 0);
    Ok(core)
}

/// Step 0x143b0cb60 (rcx core, rdx cfg, xmm2 dtSim, xmm3 r)
pub fn step(core: &mut Core, _cfg: &Cfg, dt_sim: f32, r: f32, rng: &mut Rng) {
    if !(dt_sim >= 1e-8_f64 as f32) {
        return;
    }
    let n = core.f674;
    let inv_n = 1.0 / (n as f32);
    let h = dt_sim * inv_n;
    let inv_h = 1.0 / h;
    let g0 = core.f3848.gravity;
    let gs = [g0[0] * core.ff0[0], g0[1] * core.ff0[1], g0[2] * core.ff0[2]];
    let q = core.fd0;
    let t = mul(cross(&q, &gs), 2.0);
    let c2 = cross(&q, &t);
    let g = [(c2[0] + (q[3] * t[0])) + gs[0], (c2[1] + (q[3] * t[1])) + gs[1], (c2[2] + (q[3] * t[2])) + gs[2]];
    let ground = coll::step_ground_query(core);
    let (wind_v, preset_wind) = wind::step_wind_143b0ce63(core);
    for _sub in 0..n {
        if !core.f3778.f0.is_empty() {
            wind::fn_143b123b0(r, core, h, inv_h, inv_n, g, wind_v, &preset_wind, rng);
        }
        if ground.valid && core.f65d != 0 {
            coll::fn_143b19d80(core, ground.ok, ground.pos, ground.normal);
        }
        if !core.f3778.f10.is_empty() {
            wind::fn_143ae5160(&mut core.f3778, h, inv_h);
        }
        // scene.f28 (plane groups) is never filled: shape matching (0x143b0c170) not transcribed (none in Sora)
        for _it in 0..core.f678 {
            for _li in 0..core.f67c {
                for p in core.f3778.f0.iter_mut() {
                    p.fb8 = 0;
                    p.fac = [0.0, 0.0, 0.0];
                    p.fa0 = [0.0, 0.0, 0.0];
                }
                wind::fn_143ae53f0(r, &mut core.f3778);
                for p in core.f3778.f0.iter_mut() {
                    if p.f276 == 0 && core.f804[p.f2b8] > 1 && p.fb8 > 0 {
                        let mut k = 1.0 / (p.fb8 as f32);
                        p.f0 = [(k * p.fac[0]) + p.f0[0], (k * p.fac[1]) + p.f0[1], (k * p.fac[2]) + p.f0[2]];
                        k = 1.0 / (p.fb8 as f32);
                        p.f58 = [(k * p.fa0[0]) + p.f58[0], (k * p.fa0[1]) + p.f58[1], (k * p.fa0[2]) + p.f58[2]];
                    }
                }
            }
            for i in 0..core.f3778.f748.len() {
                let root = core.f3778.f748[i];
                if core.f3778.f0[root].f240.is_none() {
                    for k in 0..core.f3778.f0[root].f1a8 {
                        let l = core.f3778.f0[root].f1b0[k];
                        chain::fn_143b192a0(r, l, core);
                    }
                }
            }
            if !core.f3778.f10.is_empty() {
                wind::fn_143ae5160(&mut core.f3778, h, inv_h);
            }
        }
        // LRA pass (none in Sora)
        for i in 0..core.f3778.f0.len() {
            let ps = &mut core.f3778.f0;
            let Some(a) = ps[i].f220 else { continue };
            if ps[i].f276 != 0 {
                continue;
            }
            let (dx, dy, dz) = (ps[i].f0[0] - ps[a].f0[0], ps[i].f0[1] - ps[a].f0[1], ps[i].f0[2] - ps[a].f0[2]);
            let l2 = ((dy * dy) + (dx * dx)) + (dz * dz);
            let p = &mut ps[i];
            if p.f228 * p.f228 < l2 {
                let len = sqrt(l2);
                let inv = 1.0 / len;
                let m = (r * p.f22c) * (len - p.f228);
                let c = [(inv * dx) * m, (inv * dy) * m, (inv * dz) * m];
                p.f0 = [p.f0[0] - c[0], p.f0[1] - c[1], p.f0[2] - c[2]];
                p.fc = [p.fc[0] - (p.f230 * c[0]), p.fc[1] - (p.f230 * c[1]), p.fc[2] - (p.f230 * c[2])];
            }
        }
        // scene.f780 (reaction transfer) is never filled: pass not transcribed (none in Sora)
        coll::step_collision(core, &ground);
        kdi::fn_143b0cb60_kdi_pass(&mut core.f3778);
        kdi::fn_143b15610(r, core);
        wind::fn_143b1e710(core, inv_h);
    }
    core.f3778.f7ac = 0;
}

/// CoreUpdate 0x143b0efc0 (rcx core, rdx cfg, xmm2 dt)
pub fn core_update(core: &mut Core, cfg: &Cfg, dt: f32, rng: &mut Rng, stats: &mut Stats) {
    let small = 1e-8_f64 as f32;
    if core.f33e8 != 0 {
        // (ER69) NOT TRANSCRIBED, a FALLBACK instead of a stop (an outfit's asset reaches it; Sora's never): variable-fps correction (0x143b0ed00 / 0x143aedcc0): none
        super::unported(1);
    }
    let td = core.f3848.time_dilation;
    let ts = bcore::time_step(core, cfg, dt, td);
    let (dt_sim, r, dtp, step_before) = (ts.dt_sim, ts.r, ts.dtp, ts.step);
    if cfg.f53 == 0 {
        return;
    }
    // global weight
    let mut time = core.f3700;
    if dtp <= time {
        if dtp > small {
            if core.f3704 != 0 && core.f36f8 < 0.0001_f64 as f32 && core.f36fc >= 0.0001_f64 as f32 {
                reset::fn_143b01210(core, 1);
                time = core.f3700;
                core.f3704 = 0;
            }
            let mut v = (((core.f36fc - core.f36f8) * dtp) / time) + core.f36f8;
            if v >= 0.0 {
                if v >= 1.0 {
                    v = 1.0;
                }
            } else {
                v = 0.0;
            }
            core.f36f8 = v;
            core.f3700 = time - dtp;
        }
    } else {
        core.f36f8 = core.f36fc;
        core.f3700 = 0.0;
        core.f3704 = 0;
    }
    for g in 0..core.f410 {
        if core.f8c4[g] != 0 {
            // (ER69) NOT TRANSCRIBED, a FALLBACK instead of a stop (an outfit's asset reaches it; Sora's never): LOD pose blend 0x143afe170: none
            super::unported(2);
        }
    }
    reset::fn_143b0e790(core);
    if cfg.f55 != 0 {
        bcore::fn_143b10010(core, dtp);
        bcore::fn_143b11700(core);
        reset::fn_143b1da00(core);
        reset::fn_143afaf80(core);
        if core.f33f6 != 0 {
            reset::fn_143afd380(core);
        }
        if core.f33f4 != 0 {
            stats.resets += 1;
            let hard = core.f33f5;
            reset::fn_143b01210(core, hard);
        }
        if core.f65c != 0 {
            reset::fn_143aee280(core, if dtp > small { r } else { 0.0 });
            coll::fn_143b10230(core, if dtp > small { dt_sim } else { 0.0 });
            if core.f33ea == 0 {
                if dtp > small {
                    step(core, cfg, dt_sim, r, rng);
                    stats.steps += 1;
                }
            } else {
                core.f33ec = core.f33ec * ((if step_before < core.f383c { 1 } else { 0 }) + 1);
                core.f670 = 0.0;
                for i in 0..core.f33ec {
                    if i > 0 {
                        core.f670 = 1.0;
                        coll::fn_143b10230(core, dt_sim);
                    }
                    step(core, cfg, dt_sim, r, rng);
                    stats.steps += 1;
                }
                core.f33ea = 0;
                core.f33ec = 0;
                core.f670 = 1.0;
            }
            chain::fn_143b0c600(core);
            for i in 0..core.f3778.f748.len() {
                let root = core.f3778.f748[i];
                let rp = &core.f3778.f0[root];
                if core.f804[rp.f2b8] > 1 && rp.f277 == 0 {
                    let kin = match rp.f198 {
                        Some(c) => core.f3778.f0[c].f276,
                        None => rp.f276,
                    };
                    if kin == 0 {
                        let target = rp.f160;
                        chain::fn_143b13b70(core, root, target);
                    }
                }
            }
            chain::fn_143af3f70(core);
            core.f33f4 = 0;
            core.f33f5 = 0;
            core.f33f6 = 0;
        }
        let bones = &core.f3848.bones;
        for p in core.f3778.f0.iter_mut() {
            p.f30 = bones[p.f248].q;
        }
    }
    hfps::fn_143afc920(core);
    let bones = &core.f3848.bones;
    for p in core.f3778.f0.iter_mut() {
        let b = &bones[p.f248];
        p.fc0 = b.q;
        p.fd0 = b.t;
        p.fe0 = b.s;
    }
    if core.f3840 == 0 {
        hfps::fn_143aefb30(core);
    }
    if dtp > small {
        core.f3714 = dt_sim + core.f3714;
    }
}
