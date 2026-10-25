//! KineDriver: Square Enix's helper-bone driver (docs/sora-animation-spec.md section 13; change
//! list B3). 313 operators run once per frame, in asset order, in place on the component-space
//! pose, after the anim graph (leg IK included). It writes 102 of Sora's 274 bones: the `_spo`
//! correctives, hair roots' scale, skirt roots, sleeve chains, hood, shoes, cuffs.
//!
//! A port of the reference evaluator `tools/lib/kd_eval.js` (notes: `native_F_kinedriver.md`),
//! in single precision with UE's polynomial `Atan2` / `SinCos`. Data: `data/kinedriver.json` of
//! the pack. Bone indices in the data are the skeleton's.

use glam::{Quat, Vec3};

use crate::blend::slerp_not_normalised;
use crate::json::Json;
use crate::Trs;

/// 0x3F3504F3, as the exe has it.
const S: f32 = 0.707_106_77;

fn quat(v: &Json) -> Quat {
    let f = v.floats();
    Quat::from_xyzw(f[0], f[1], f[2], f[3])
}

fn vec3(v: &Json) -> Vec3 {
    let f = v.floats();
    Vec3::new(f[0], f[1], f[2])
}

/// `qnorm` of the evaluator.
fn normalise(q: Quat) -> Quat {
    let square = q.length_squared();
    if square < 1.0e-8 {
        Quat::IDENTITY
    } else {
        q * (1.0 / square.sqrt())
    }
}

fn normalise_if_needed(q: Quat) -> Quat {
    if (1.0 - q.length_squared()).abs() >= 0.01 {
        normalise(q)
    } else {
        q
    }
}

/// `FQuat::RotateVector`, for quaternions that need not be unit length.
fn rotate(q: Quat, v: Vec3) -> Vec3 {
    let u = Vec3::new(q.x, q.y, q.z);
    let t = 2.0 * u.cross(v);
    v + q.w * t + u.cross(t)
}

fn mul(a: Quat, b: Quat) -> Quat {
    Quat::from_xyzw(
        a.w * b.x + a.x * b.w + a.y * b.z - a.z * b.y,
        a.w * b.y - a.x * b.z + a.y * b.w + a.z * b.x,
        a.w * b.z + a.x * b.y - a.y * b.x + a.z * b.w,
        a.w * b.w - a.x * b.x - a.y * b.y - a.z * b.z,
    )
}

fn conjugate(q: Quat) -> Quat {
    Quat::from_xyzw(-q.x, -q.y, -q.z, q.w)
}

fn unit(v: Vec3) -> Vec3 {
    let l = v.length_squared();
    if l == 1.0 {
        v
    } else if l < 1.0e-8 {
        Vec3::ZERO
    } else {
        v * (1.0 / l.sqrt())
    }
}

/// UE component-space quaternion -> KineDriver space.
fn to_kd(q: Quat) -> Quat {
    let r = mul(Quat::from_xyzw(S, 0.0, 0.0, S), q);
    Quat::from_xyzw(r.x, -r.y, r.z, -r.w)
}

/// KineDriver-space quaternion -> UE component space, normalised.
fn from_kd(q: Quat) -> Quat {
    normalise(mul(Quat::from_xyzw(-S, 0.0, 0.0, S), Quat::from_xyzw(q.x, -q.y, q.z, -q.w)))
}

fn swap_yz(v: Vec3) -> Vec3 {
    Vec3::new(v.x, v.z, v.y)
}

/// `FMath::Atan2` (0x142CE9900).
fn atan2_fast(y: f32, x: f32) -> f32 {
    let (ax, ay) = (x.abs(), y.abs());
    let big = ay > ax;
    let (t0, t1) = if big { (ay, ax) } else { (ax, ay) };
    if t0 == 0.0 {
        return 0.0;
    }
    let mut t3 = t1 / t0;
    let t4 = t3 * t3;
    let mut p = 7.212_885_4e-3;
    p = p * t4 + -3.505_968e-2;
    p = p * t4 + 8.167_588e-2;
    p = p * t4 + -1.337_465_7e-1;
    p = p * t4 + 1.985_656_4e-1;
    p = p * t4 + -3.332_5e-1;
    p = p * t4 + 1.0;
    t3 *= p;
    if big {
        t3 = std::f32::consts::FRAC_PI_2 - t3;
    }
    if x < 0.0 {
        t3 = std::f32::consts::PI - t3;
    }
    if y < 0.0 {
        t3 = -t3;
    }
    t3
}

/// `FMath::SinCos`.
fn sin_cos_fast(v: f32) -> (f32, f32) {
    let mut q = 0.159_154_94 * v;
    q = if v >= 0.0 { (q + 0.5).trunc() } else { (q - 0.5).trunc() };
    let mut y = v - 6.283_185_5 * q;
    let mut sign = 1.0;
    if y > 1.570_796_4 {
        y = 3.141_592_7 - y;
        sign = -1.0;
    } else if y < -1.570_796_4 {
        y = -3.141_592_7 - y;
        sign = -1.0;
    }
    let y2 = y * y;
    let s = (((((-2.388_985_9e-8 * y2 + 2.752_556_2e-6) * y2 - 0.000_198_408_74) * y2 + 0.008_333_331) * y2 - 0.166_666_67) * y2 + 1.0) * y;
    let c = sign * (((((-2.605_161_5e-7 * y2 + 2.476_049_5e-5) * y2 - 0.001_388_837_8) * y2 + 0.041_666_638) * y2 - 0.5) * y2 + 1.0);
    (s, c)
}

/// 0x143AC2A10: the rotation taking unit vector `a` onto unit vector `b`.
fn find_between(a: Vec3, b: Vec3) -> Quat {
    let d = a.dot(b);
    if d >= 1.0 {
        return Quat::IDENTITY;
    }
    let q = if d > -1.0 {
        let s = (d + d + 2.0).sqrt();
        let k = 1.0 / s;
        let c = a.cross(b);
        Quat::from_xyzw(c.x * k, c.y * k, c.z * k, s * 0.5)
    } else {
        let mut l = (a.y * a.y + a.x * a.x).sqrt();
        let (mut x, mut y, mut z) = (0.0, a.x, -a.y);
        if l < 0.0001 {
            (x, y, z) = (-a.z, 0.0, a.x);
            l = (a.z * a.z + a.x * a.x).sqrt();
        }
        Quat::from_xyzw(x / l, y / l, z / l, 0.0)
    };
    normalise(q)
}

/// (party A2) `FQuat::FQuat(const FMatrix&)` (0x140511D20, stock 4.17 by shape): `m[i]` = row i (UE row-vector
/// matrices); identity when an axis row is nearly zero (|each component| <= 1e-4).
fn matrix_to_quat(m: [Vec3; 3]) -> Quat {
    if m.iter().any(|r| r.x.abs() <= 1.0e-4 && r.y.abs() <= 1.0e-4 && r.z.abs() <= 1.0e-4) {
        return Quat::IDENTITY;
    }
    let tr = m[0].x + m[1].y + m[2].z;
    if tr > 0.0 {
        let inv_s = 1.0 / (tr + 1.0).sqrt();
        let s = 0.5 * inv_s;
        return Quat::from_xyzw((m[1].z - m[2].y) * s, (m[2].x - m[0].z) * s, (m[0].y - m[1].x) * s, 0.5 / inv_s);
    }
    let at = |i: usize, j: usize| m[i][j];
    let mut i = 0;
    if at(1, 1) > at(0, 0) {
        i = 1;
    }
    if at(2, 2) > at(i, i) {
        i = 2;
    }
    const NEXT: [usize; 3] = [1, 2, 0];
    let (j, k) = (NEXT[i], NEXT[NEXT[i]]);
    let s = at(i, i) - at(j, j) - at(k, k) + 1.0;
    let inv_s = 1.0 / s.sqrt();
    let mut q = [0.0f32; 4];
    q[i] = 0.5 / inv_s;
    let s = 0.5 * inv_s;
    q[3] = (at(j, k) - at(k, j)) * s;
    q[j] = (at(i, j) + at(j, i)) * s;
    q[k] = (at(i, k) + at(k, i)) * s;
    Quat::from_xyzw(q[0], q[1], q[2], q[3])
}

fn base_scale(s: Vec3) -> Vec3 {
    if s == Vec3::ZERO {
        Vec3::ONE
    } else {
        s
    }
}

/// The scalar parameters an operator can own, by slot.
const PARAMS: [&str; 12] = ["RotateAngle", "BendingAngle", "BendS", "BendT", "Roll", "TranslateX", "TranslateY", "TranslateZ", "ScaleX", "ScaleY", "ScaleZ", "QuatAngle"];
const ROTATE_ANGLE: usize = 0;
const BENDING_ANGLE: usize = 1;
const BEND_S: usize = 2;
const BEND_T: usize = 3;
const ROLL: usize = 4;
const TRANSLATE: usize = 5;
const SCALE: usize = 8;
const QUAT_ANGLE: usize = 11;

fn param_slot(name: &str) -> Option<usize> {
    PARAMS.iter().position(|p| *p == name)
}

/// An operator's parameter storage: values persist between frames as in the runtime objects.
#[derive(Clone, Copy, Debug, Default)]
struct Params {
    values: [f32; 12],
    /// Which slots this operator has.
    has: u16,
    rotate_quat: Quat,
    bending_quat: Quat,
}

impl Params {
    fn set(&mut self, slot: usize, value: f32) {
        self.values[slot] = value;
        self.has |= 1 << slot;
    }

    fn owns(&self, slot: usize) -> bool {
        self.has & (1 << slot) != 0
    }
}

#[derive(Clone, Debug)]
struct Curve {
    px0: f32,
    vx1_0: f32,
    vx2_1: f32,
    grad0: f32,
    grad1: f32,
    py0: f32,
    py0a: f32,
    py0b: f32,
    py1: f32,
    py1a: f32,
    py1b: f32,
    py2: f32,
    by_coef: bool,
}

#[derive(Clone, Debug)]
enum Kind {
    SourceRotate { sources: Vec<usize>, weights: Vec<f32>, base: Option<usize>, neutral_rotate: Quat, aim: Vec3, up: Vec3 },
    Link { in_op: usize, in_param: Option<usize>, out_op: usize, out_param: Option<usize>, curve: Curve },
    LinkLinear { in_op: usize, in_param: Option<usize>, out_op: usize, out_param: Option<usize>, scale: f32, offset: f32, min: Option<f32>, max: Option<f32> },
    BendSTRoll { target: usize, base: Option<usize>, neutral_rotate: Quat, aim: Vec3, up: Vec3, cross: Vec3 },
    /// `TargetBendRoll` (`roll` true) and `TargetRotate`.
    BendRoll { target: usize, base: Option<usize>, neutral_rotate: Quat, aim: Vec3, source_quat_op: Option<usize>, rotate_source: bool, roll: bool },
    Translate { target: usize, base: Option<usize>, neutral_translate: Vec3, neutral_rotate: Quat },
    Scale { target: usize, target_parent: Option<usize>, base: Option<usize>, clamp_zero: bool, compensate: bool },
    /// `ComputeSpaceBases` (0x143ab0480); `compensate` = the operator's IsScaleOpChild AND the user data's
    /// EnableScaleOpChildSSC (party A: Goofy's shortEar data; native_F_kinedriver.md 4.8, Alpha 1: l.S / P.S).
    SpaceBases { target: usize, parent: usize, compensate: bool },
    /// (party A, Donald's and Goofy's assets) `SourceOther` (vtable 0x1474FE310): Compute 0x143AB3580 copies the source
    /// bone's component-space Scale3D into its parameters, GetParamPtr 0x143AC1C80 exposes them as ScaleX / Y / Z [C].
    SourceOther { source: usize },
    /// (ER73, the outfits' assets) `ConnectEquals` (0x143ab0f40): out = coef * in.
    Equals { in_op: usize, in_param: Option<usize>, out_op: usize, out_param: Option<usize>, coef: f32 },
    /// (ER73) `TargetPoscns` (0x143abc500), the path without orient / scale affect: the target's translation = the
    /// weighted mean of (source translation + offset), + the offset translate (offsets in KineDriver space).
    Poscns { target: usize, sources: Vec<usize>, weights: Vec<f32>, offsets: Vec<Vec3>, offset_translate: Vec3 },
    /// (ER73; party A2: TRANSCRIBED from the decompiled C of 0x143ab97b0, lines cited in `evaluate`) `TargetDircns`.
    /// Without an up object: the frame = KD(parent rotation) x NeutralRotate; the aim point (aim object + its rotated,
    /// scaled AimSrcOffset) brought into that frame; the shortest arc from AimVector to it (0x143AC2A10) after the frame.
    /// With an up object: the rotation is the basis (aim dir, up dir, cross) mapped from (AimVector, UpVector,
    /// CrossVector) (FQuat(FMatrix) 0x140511D20); the parent and the neutral rotate are not read.
    /// Runtime layout: +0xC target, +0x10 parent, +0x14 aim, +0x18 up, +0x20 NeutralRotate, +0x30 AimVector, +0x3C
    /// UpVector, +0x48 CrossVector, +0x54 AimSrcOffset, +0x60 UpSrcOffset, +0x6C AimTransformAffect, +0x6D UpTransformAffect.
    Dircns { target: usize, parent: Option<usize>, aim_object: usize, up_object: Option<usize>, neutral_rotate: Quat, aim: Vec3, up: Vec3, cross: Vec3, aim_offset: Vec3, up_offset: Vec3, aim_affect: bool, up_affect: bool },
}

/// The driver of one character.
#[derive(Clone, Debug)]
pub struct KineDriver {
    operators: Vec<Kind>,
    state: Vec<Params>,
    /// Bones the driver writes, ascending.
    written: Vec<usize>,
}

impl KineDriver {
    /// From the text of `data/kinedriver.json`.
    pub fn from_json(data: &Json) -> Result<KineDriver, String> {
        let index = |v: &Json| v.as_i64().filter(|i| *i >= 0).map(|i| i as usize);
        let f = |o: &Json, key: &str| o.get(key).as_f32().unwrap_or(0.0);
        let mut operators = Vec::new();
        let mut state = Vec::new();
        for (i, o) in data.get("operators").items().iter().enumerate() {
            if o.get("index").as_usize() != Some(i) {
                return Err(format!("kinedriver operator {i}: out of order"));
            }
            let kind = o.get("type").as_str().unwrap_or("");
            let mut p = Params::default();
            let target = || index(o.get("target")).ok_or(format!("kinedriver operator {i}: no target"));
            let op = match kind {
                "SourceRotate" => {
                    for slot in [ROTATE_ANGLE, BENDING_ANGLE, BEND_S, BEND_T, ROLL] {
                        p.set(slot, 0.0);
                    }
                    p.rotate_quat = Quat::from_xyzw(0.0, 0.0, 0.0, 0.0);
                    p.bending_quat = Quat::from_xyzw(0.0, 0.0, 0.0, 0.0);
                    Kind::SourceRotate {
                        sources: o.get("sources").items().iter().filter_map(index).collect(),
                        weights: o.get("weights").floats(),
                        base: index(o.get("base")),
                        neutral_rotate: quat(o.get("neutral_rotate")),
                        aim: vec3(o.get("aim")),
                        up: vec3(o.get("up")),
                    }
                }
                "ConnectEZParamLink" => Kind::Link {
                    in_op: index(o.get("in_op")).ok_or("link without in_op")?,
                    in_param: o.get("in_param").as_str().and_then(param_slot),
                    out_op: index(o.get("out_op")).ok_or("link without out_op")?,
                    out_param: o.get("out_param").as_str().and_then(param_slot),
                    curve: Curve {
                        px0: f(o, "PX0"),
                        vx1_0: f(o, "VX1_0"),
                        vx2_1: f(o, "VX2_1"),
                        grad0: f(o, "Grad0"),
                        grad1: f(o, "Grad1"),
                        py0: f(o, "PY0"),
                        py0a: f(o, "PY0A"),
                        py0b: f(o, "PY0B"),
                        py1: f(o, "PY1"),
                        py1a: f(o, "PY1A"),
                        py1b: f(o, "PY1B"),
                        py2: f(o, "PY2"),
                        by_coef: o.get("by_coef").as_bool().unwrap_or(false),
                    },
                },
                "ConnectEZParamLinkLinear" => Kind::LinkLinear {
                    in_op: index(o.get("in_op")).ok_or("link without in_op")?,
                    in_param: o.get("in_param").as_str().and_then(param_slot),
                    out_op: index(o.get("out_op")).ok_or("link without out_op")?,
                    out_param: o.get("out_param").as_str().and_then(param_slot),
                    scale: f(o, "scale"),
                    offset: f(o, "offset"),
                    min: o.get("enable_min").as_bool().unwrap_or(false).then(|| f(o, "clamp_min")),
                    max: o.get("enable_max").as_bool().unwrap_or(false).then(|| f(o, "clamp_max")),
                },
                "TargetBendSTRoll" => {
                    p.set(BEND_S, f(o, "bend_s"));
                    p.set(BEND_T, f(o, "bend_t"));
                    p.set(ROLL, f(o, "roll"));
                    Kind::BendSTRoll { target: target()?, base: index(o.get("base")), neutral_rotate: quat(o.get("neutral_rotate")), aim: vec3(o.get("aim")), up: vec3(o.get("up")), cross: vec3(o.get("cross")) }
                }
                "TargetBendRoll" | "TargetRotate" => {
                    let roll = kind == "TargetBendRoll";
                    p.set(QUAT_ANGLE, f(o, "quat_weight"));
                    if roll {
                        p.set(ROLL, f(o, "roll"));
                    }
                    if o.get("as_quat_angle").as_bool().unwrap_or(false) && index(o.get("source_quat_op")).is_some() {
                        return Err(format!("kinedriver operator {i}: the AsQuatAngle path (0x143AC2C30) is not modelled"));
                    }
                    Kind::BendRoll {
                        target: target()?,
                        base: index(o.get("base")),
                        neutral_rotate: quat(o.get("neutral_rotate")),
                        aim: if roll { vec3(o.get("aim")) } else { Vec3::X },
                        source_quat_op: index(o.get("source_quat_op")),
                        rotate_source: !roll,
                        roll,
                    }
                }
                "TargetTranslate" => {
                    let t = vec3(o.get("translate"));
                    for (k, v) in [t.x, t.y, t.z].into_iter().enumerate() {
                        p.set(TRANSLATE + k, v);
                    }
                    Kind::Translate { target: target()?, base: index(o.get("base")), neutral_translate: vec3(o.get("neutral_translate")), neutral_rotate: quat(o.get("neutral_rotate")) }
                }
                "TargetScale" => {
                    let s = vec3(o.get("scale"));
                    for (k, v) in [s.x, s.y, s.z].into_iter().enumerate() {
                        p.set(SCALE + k, v);
                    }
                    Kind::Scale {
                        target: target()?,
                        target_parent: index(o.get("target_parent")),
                        base: index(o.get("base")),
                        clamp_zero: o.get("clamp_zero").as_bool().unwrap_or(false),
                        compensate: o.get("segment_scale_compensate").as_bool().unwrap_or(false),
                    }
                }
                "ConnectEquals" => Kind::Equals {
                    in_op: index(o.get("in_op")).ok_or("link without in_op")?,
                    in_param: o.get("in_param").as_str().and_then(param_slot),
                    out_op: index(o.get("out_op")).ok_or("link without out_op")?,
                    out_param: o.get("out_param").as_str().and_then(param_slot),
                    coef: o.get("coef").as_f32().unwrap_or(1.0),
                },
                "TargetPoscns" => Kind::Poscns {
                    target: target()?,
                    sources: o.get("sources").items().iter().filter_map(index).collect(),
                    weights: o.get("weights").floats(),
                    offsets: o.get("offsets").items().iter().map(vec3).collect(),
                    offset_translate: vec3(o.get("offset_translate")),
                },
                "TargetDircns" => Kind::Dircns {
                    target: target()?,
                    parent: index(o.get("parent")),
                    aim_object: index(o.get("aim_object")).ok_or("Dircns without an aim object")?,
                    up_object: index(o.get("up_object")),
                    neutral_rotate: quat(o.get("neutral_rotate")),
                    aim: vec3(o.get("aim")),
                    up: vec3(o.get("up")),
                    cross: vec3(o.get("cross")),
                    aim_offset: vec3(o.get("aim_offset")),
                    up_offset: vec3(o.get("up_offset")),
                    aim_affect: o.get("aim_transform_affect").as_bool().unwrap_or(false),
                    up_affect: o.get("up_transform_affect").as_bool().unwrap_or(false),
                },
                "ComputeSpaceBases" => Kind::SpaceBases {
                    target: target()?,
                    parent: index(o.get("parent")).ok_or("ComputeSpaceBases without a parent")?,
                    compensate: o.get("is_scale_op_child").as_bool().unwrap_or(false) && data.get("settings").get("EnableScaleOpChildSSC").as_bool().unwrap_or(false),
                },
                "SourceOther" => {
                    for k in 0..3 {
                        p.set(SCALE + k, 0.0);
                    }
                    Kind::SourceOther { source: index(o.get("source")).ok_or(format!("kinedriver operator {i}: SourceOther without a source"))? }
                }
                other => return Err(format!("kinedriver operator {i}: type {other} is not modelled")),
            };
            operators.push(op);
            state.push(p);
        }
        if operators.is_empty() {
            return Err("kinedriver.json has no operators".to_string());
        }
        let written = data.get("bones").items().iter().filter(|b| b.get("written").as_bool() == Some(true)).filter_map(|b| b.get("index").as_usize()).collect();
        Ok(KineDriver { operators, state, written })
    }

    pub fn operator_count(&self) -> usize {
        self.operators.len()
    }

    /// The bones the driver writes, ascending.
    pub fn written(&self) -> &[usize] {
        &self.written
    }

    /// Runs every operator, in order, on the component-space `pose` (modified in place); `local`
    /// is the animation's local pose (read by `ComputeSpaceBases`).
    pub fn evaluate(&mut self, pose: &mut [Trs], local: &[Trs]) {
        let state = &mut self.state;
        for (i, op) in self.operators.iter().enumerate() {
            match op {
                Kind::SourceRotate { sources, weights, base, neutral_rotate, aim, up } => {
                    let (mut acc, mut sum) = (Quat::IDENTITY, 0.0);
                    for (source, weight) in sources.iter().zip(weights) {
                        sum += weight;
                        acc = normalise(slerp_not_normalised(acc, to_kd(pose[*source].rotation), weight / sum));
                    }
                    let b = match base {
                        Some(base) => mul(normalise_if_needed(to_kd(pose[*base].rotation)), *neutral_rotate),
                        None => *neutral_rotate,
                    };
                    let l = mul(conjugate(b), acc);
                    let p = &mut state[i];
                    p.rotate_quat = l;
                    p.values[ROTATE_ANGLE] = 2.0 * l.w.abs().min(1.0).acos();
                    let v = unit(rotate(l, *aim));
                    let c = v.dot(*aim).clamp(-1.0, 1.0);
                    p.values[BENDING_ANGLE] = c.acos();
                    let bq = find_between(*aim, v);
                    p.bending_quat = bq;
                    let n = unit(aim.cross(*up));
                    p.values[BEND_S] = 2.0 * atan2_fast(-n.dot(v), c + 1.0);
                    p.values[BEND_T] = 2.0 * atan2_fast(up.dot(v), c + 1.0);
                    let twist = mul(conjugate(bq), l);
                    let (ax, ay, az) = (aim.x.abs(), aim.y.abs(), aim.z.abs());
                    let k = if ax <= ay {
                        if az < ay {
                            1
                        } else {
                            2
                        }
                    } else if az < ax {
                        0
                    } else {
                        2
                    };
                    let mut roll = 2.0 * atan2_fast([twist.x, twist.y, twist.z][k] / aim[k], twist.w);
                    if roll < -3.141_592_7 {
                        roll += 6.283_185_5;
                    } else if roll > 3.141_592_7 {
                        roll -= 6.283_185_5;
                    }
                    p.values[ROLL] = roll;
                }
                Kind::Link { in_op, in_param, out_op, out_param, curve: o } => {
                    let (Some(input), Some(output)) = (*in_param, *out_param) else { continue };
                    if !state[*in_op].owns(input) || !state[*out_op].owns(output) {
                        continue;
                    }
                    let x = state[*in_op].values[input];
                    let d = x - o.px0;
                    let a0 = ((o.py0a * 3.0 - o.py0) - o.py0b * 3.0) + o.py1;
                    let b0 = (o.py0 * 3.0 - o.py0a * 6.0) + o.py0b * 3.0;
                    let c0 = o.py0a * 3.0 - o.py0 * 3.0;
                    let a1 = ((o.py1a * 3.0 - o.py1) - o.py1b * 3.0) + o.py2;
                    let b1 = (o.py1 * 3.0 - o.py1a * 6.0) + o.py1b * 3.0;
                    let c1 = o.py1a * 3.0 - o.py1 * 3.0;
                    let mut y = if !(d > 0.0) {
                        d * o.grad0 + o.py0
                    } else {
                        let d2 = d - o.vx1_0;
                        if !(d2 > 0.0) {
                            let t = d / o.vx1_0;
                            ((t * a0 + b0) * t + c0) * t + o.py0
                        } else {
                            let d3 = d2 - o.vx2_1;
                            if d3 >= 0.0 {
                                d3 * o.grad1 + o.py2
                            } else {
                                let t = d2 / o.vx2_1;
                                ((t * a1 + b1) * t + c1) * t + o.py1
                            }
                        }
                    };
                    if o.by_coef {
                        y *= x;
                    }
                    state[*out_op].values[output] = y;
                }
                Kind::LinkLinear { in_op, in_param, out_op, out_param, scale, offset, min, max } => {
                    let (Some(input), Some(output)) = (*in_param, *out_param) else { continue };
                    if !state[*in_op].owns(input) || !state[*out_op].owns(output) {
                        continue;
                    }
                    let mut y = scale * state[*in_op].values[input] + offset;
                    if let Some(min) = min {
                        y = y.max(*min);
                    }
                    if let Some(max) = max {
                        y = y.min(*max);
                    }
                    state[*out_op].values[output] = y;
                }
                Kind::BendSTRoll { target, base, neutral_rotate, aim, up, cross } => {
                    let p = &state[i];
                    let (ts, tt) = ((p.values[BEND_S] * 0.5).tan(), (p.values[BEND_T] * 0.5).tan());
                    let k = 2.0 / (tt * tt + ts * ts + 1.0);
                    let m = k - 1.0;
                    let direction = (tt * *up * k + m * *aim) - ts * *cross * k;
                    let mut q = find_between(*aim, direction);
                    if p.values[ROLL].abs() > 0.0001 {
                        let (s, c) = sin_cos_fast(p.values[ROLL] * 0.5);
                        q = mul(q, Quat::from_xyzw(aim.x * s, aim.y * s, aim.z * s, c));
                    }
                    let mut r = mul(*neutral_rotate, q);
                    if let Some(base) = base {
                        r = mul(normalise_if_needed(to_kd(pose[*base].rotation)), r);
                    }
                    pose[*target].rotation = from_kd(r);
                }
                Kind::BendRoll { target, base, neutral_rotate, aim, source_quat_op, rotate_source, roll } => {
                    let p = &state[i];
                    let mut q = Quat::IDENTITY;
                    if let Some(source) = source_quat_op {
                        let from = if *rotate_source { state[*source].rotate_quat } else { state[*source].bending_quat };
                        q = normalise(slerp_not_normalised(Quat::IDENTITY, from, p.values[QUAT_ANGLE]));
                    }
                    if *roll && p.values[ROLL].abs() > 0.0001 {
                        let (s, c) = sin_cos_fast(p.values[ROLL] * 0.5);
                        q = mul(q, Quat::from_xyzw(aim.x * s, aim.y * s, aim.z * s, c));
                    }
                    let mut r = mul(*neutral_rotate, q);
                    if let Some(base) = base {
                        r = mul(normalise_if_needed(to_kd(pose[*base].rotation)), r);
                    }
                    pose[*target].rotation = from_kd(r);
                }
                Kind::Translate { target, base, neutral_translate, neutral_rotate } => {
                    let p = &state[i];
                    let t = Vec3::new(p.values[TRANSLATE], p.values[TRANSLATE + 1], p.values[TRANSLATE + 2]);
                    let out = match base {
                        None => *neutral_translate + rotate(*neutral_rotate, t),
                        Some(base) => {
                            let b = pose[*base];
                            let q = normalise_if_needed(to_kd(b.rotation));
                            let s = base_scale(b.scale);
                            swap_yz(b.translation) + rotate(q, s * *neutral_translate) + rotate(mul(q, *neutral_rotate), s * t)
                        }
                    };
                    pose[*target].translation = swap_yz(out);
                }
                Kind::Scale { target, target_parent, base, clamp_zero, compensate } => {
                    let p = &state[i];
                    let mut s = Vec3::new(p.values[SCALE], p.values[SCALE + 1], p.values[SCALE + 2]);
                    if *clamp_zero {
                        s = s.max(Vec3::splat(0.0001));
                    }
                    if let Some(base) = base {
                        s *= base_scale(pose[*base].scale);
                        if *compensate && target_parent.is_some() {
                            s /= pose[*base].scale;
                        }
                    }
                    pose[*target].scale = s;
                }
                Kind::Equals { in_op, in_param, out_op, out_param, coef } => {
                    let (Some(input), Some(output)) = (*in_param, *out_param) else { continue };
                    if !state[*in_op].owns(input) || !state[*out_op].owns(output) {
                        continue;
                    }
                    state[*out_op].values[output] = coef * state[*in_op].values[input];
                }
                Kind::Poscns { target, sources, weights, offsets, offset_translate } => {
                    let (mut acc, mut sum) = (Vec3::ZERO, 0.0f32);
                    for (k, (source, weight)) in sources.iter().zip(weights).enumerate() {
                        let Some(src) = pose.get(*source) else { continue };
                        sum += weight;
                        if sum <= 0.0 {
                            continue;
                        }
                        let point = swap_yz(src.translation) + offsets.get(k).copied().unwrap_or(Vec3::ZERO);
                        acc += (point - acc) * (weight / sum);
                    }
                    if sum > 0.0 && *target < pose.len() {
                        pose[*target].translation = swap_yz(acc + *offset_translate);
                    }
                }
                Kind::Dircns { target, parent, aim_object, up_object, neutral_rotate, aim, up, cross, aim_offset, up_offset, aim_affect, up_affect } => {
                    // 0x143ab97b0 (decompiled C, line numbers of re/decomp/143AB/143AB97B0.c)
                    if *target >= pose.len() || *aim_object >= pose.len() {
                        continue;
                    }
                    let t = pose[*target].translation;
                    match up_object.filter(|u| *u < pose.len()) {
                        None => {
                            // 101-161: the parent's rotation in KD space (identity without a parent)
                            let p = parent.filter(|p| *p < pose.len()).map_or(Quat::IDENTITY, |p| normalise_if_needed(to_kd(pose[p].rotation)));
                            // 163-180: frame = P * NeutralRotate
                            let frame = mul(p, *neutral_rotate);
                            // 182-202: the aim point in UE space: the offset scaled by the aim object's (raw) scale and
                            // turned by its UE rotation, + its translation, - the target's; then KD axes (Y <-> Z)
                            let a = pose[*aim_object];
                            let v = rotate(a.rotation, *aim_offset * a.scale) + a.translation - t;
                            // 203-228: into the frame (conjugate), safe-normalised (zero below 1e-8)
                            let local = unit(rotate(conjugate(frame), swap_yz(v)));
                            // 232: shortest arc AimVector -> local (0x143AC2A10); 233-282: frame * it, back to UE
                            pose[*target].rotation = from_kd(mul(frame, find_between(*aim, local)));
                        }
                        Some(u) => {
                            // 285-351: the aim object's rotation in KD space; 352-360: its scale (0 -> 1)
                            let a = pose[*aim_object];
                            let point = |obj: &Trs, offset: Vec3, affect: bool| -> Vec3 {
                                // 362-380 / 466-484: the offset raw, or scaled and turned by the object's KD rotation
                                let off = if affect { rotate(normalise_if_needed(to_kd(obj.rotation)), offset * base_scale(obj.scale)) } else { offset };
                                // 381-383 / 485-487: + (object - target) in KD axes
                                off + swap_yz(obj.translation - t)
                            };
                            // 384-398: the aim direction
                            let ad = unit(point(&a, *aim_offset, *aim_affect));
                            let uv = point(&pose[u], *up_offset, *up_affect);
                            // 488-505: c = unit(aim x up); 506-523: b = unit(c x aim)
                            let c = unit(ad.cross(uv));
                            let b = unit(c.cross(ad));
                            // 528-552: M[i][j] = Aim[i] a[j] + Up[i] b[j] + Cross[i] c[j] (rows; row 3 = (0,0,0,1));
                            // 554: FQuat(FMatrix) (0x140511D20, stock with the zero-axis guard)
                            let row = |i: usize| ad * aim[i] + b * up[i] + c * cross[i];
                            let q = matrix_to_quat([row(0), row(1), row(2)]);
                            // 555-607: back to UE (from_kd), normalised
                            pose[*target].rotation = from_kd(q);
                        }
                    }
                }
                Kind::SourceOther { source } => {
                    let s = pose[*source].scale;
                    let p = &mut state[i];
                    p.values[SCALE] = s.x;
                    p.values[SCALE + 1] = s.y;
                    p.values[SCALE + 2] = s.z;
                }
                Kind::SpaceBases { target, parent, compensate } => {
                    // FTransform::Multiply(local, parent), rotation normalised.
                    let (mut l, p) = (local[*target], pose[*parent]);
                    if *compensate {
                        // (party A) segment scale compensation of a scale operator's child (raw division, as the exe)
                        l.scale /= p.scale;
                    }
                    pose[*target] = Trs { rotation: normalise(mul(p.rotation, l.rotation)), translation: rotate(p.rotation, p.scale * l.translation) + p.translation, scale: l.scale * p.scale };
                }
            }
        }
    }
}
