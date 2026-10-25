//! The engine-free half of the character look (Sora and the Kingdom Key shaded by the game's own
//! material code): everything a host needs between `PACK/materials/materials.json`
//! (tools/sora_material_pack.js) and the bytes of the shader's uniform block, without Bevy and
//! without a GPU API. Two hosts use it: the Bevy test app (`kh-app/src/char_material.rs`) and the
//! Elden Ring renderer (`kh-er-render`). A fix of the look is made here (or in the translator) once.
//!
//! - [`Expr`] / [`OwnerCb`]: the uniform expressions of a material's constant buffer, evaluated as
//!   stock UE 4.17 evaluates them.
//! - [`Collection`], [`Khs`], [`KhsState`]: the two material parameter collections and the presets
//!   of the post-process material KHS.
//! - [`BoneParam`]: the mesh asset's bone pose -> material parameter table.
//! - [`Rig`], [`Spot`], [`Env`]: the light rig and the reflection environment (STAND-INS for the
//!   game's lights; the values of a host are the host's).
//! - [`CHAR_COMMON_WGSL`]: the harness text both hosts put in front of their own fragment entry
//!   (the structs the translated shaders use, the light shader, the reflection look-up, and
//!   `kh_shade`, the composition of one pixel).
//! - [`Params`]: the uniform block as plain rows, for a host that uploads bytes itself.
//! - [`library`]: materials.json read into per-slot records for such a host.
//!
//! What is the game's and what is a stand-in: `build/sora_tmp/IMPL_CHARMAT_STATE.md`.

pub mod library;

use std::collections::HashMap;

use glam::{Vec3, Vec4};
use serde_json::Value;

pub const MAX_CB: usize = 160;
pub const MAX_COLLECTION: usize = 48;
pub const MAX_TEXTURES: usize = 14;
/// Rows of the post-process material's collection (KHS_RefKit_MatParamCol).
pub const KHS_ROWS: usize = 18;
/// The discernment mask a primitive writes when its own value is 0: 1 | 512 (the base pass
/// reads Primitive row 17; 512 = "the post-process material composes this pixel").
pub const DISCERNMENT: f32 = 513.0 / 65536.0;
/// The world's KHS preset when a host does not say: Twilight Town's controller (a "KH World"
/// whose sun, intensity 3, is the nearest to the test level's key light).
pub const KHS_WORLD_PRESET: &str = "twilight";
/// The camp menu's pictures are rendered in the controller's camp mode.
pub const KHS_MENU_PRESET: &str = "camp";

/// The harness text shared by the hosts (see the file's head).
pub const CHAR_COMMON_WGSL: &str = include_str!("../shaders/char_common.wgsl");

// ---------------------------------------------------------------------------- uniform expressions

/// A uniform expression of a shader map (tools/sora_material_pack.js `tree`).
#[derive(Clone, Debug)]
pub enum Expr {
    Const([f32; 4]),
    Scalar(String, f32),
    Vector(String, [f32; 4]),
    Time,
    Op(Op, Vec<Expr>),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Op {
    Add,
    Sub,
    Mul,
    Div,
    Dot(usize),
    Append(usize),
    Swizzle([i32; 4], usize),
    Clamp,
    Min,
    Max,
    Fmod,
    Sin,
    Cos,
    Sqrt,
    Frac,
    Floor,
    Ceil,
    Abs,
    Saturate,
    Round,
    Sign,
    Truncate,
    /// (RSF round 20) UE's FMaterialUniformExpressionTrigMath (0 sin, 1 cos, 2 tan, 3 asin, 4 acos, 5 atan, 6 atan2(x, y)),
    /// the translator's `trig0` .. `trig6`: a time-dependent tree with one was 0 here
    Trig(u8),
    Bad,
}

pub fn four(v: &Value) -> [f32; 4] {
    let mut out = [0.0; 4];
    for (k, x) in v.as_array().into_iter().flatten().take(4).enumerate() {
        out[k] = x.as_f64().unwrap_or(0.0) as f32;
    }
    out
}

impl Expr {
    pub fn parse(v: &Value) -> Expr {
        if let Some(c) = v.get("c") {
            return Expr::Const(four(c));
        }
        if let Some(name) = v.get("s").and_then(Value::as_str) {
            return Expr::Scalar(name.to_string(), v.get("d").and_then(Value::as_f64).unwrap_or(0.0) as f32);
        }
        if let Some(name) = v.get("v").and_then(Value::as_str) {
            return Expr::Vector(name.to_string(), v.get("d").map(four).unwrap_or_default());
        }
        if v.get("t").is_some() {
            return Expr::Time;
        }
        let n = v.get("n").and_then(Value::as_u64).unwrap_or(4) as usize;
        let op = match v.get("f").and_then(Value::as_str).unwrap_or("") {
            "add" => Op::Add,
            "sub" => Op::Sub,
            "mul" => Op::Mul,
            "div" => Op::Div,
            "dot" => Op::Dot(n),
            "append" => Op::Append(n),
            "swizzle" => {
                let mut index = [-1; 4];
                for (k, x) in v.get("i").and_then(Value::as_array).into_iter().flatten().take(4).enumerate() {
                    index[k] = x.as_i64().unwrap_or(-1) as i32;
                }
                Op::Swizzle(index, n)
            }
            "clamp" => Op::Clamp,
            "min" => Op::Min,
            "max" => Op::Max,
            "fmod" => Op::Fmod,
            "sin" => Op::Sin,
            "cos" => Op::Cos,
            "sqrt" => Op::Sqrt,
            "frac" => Op::Frac,
            "floor" => Op::Floor,
            "ceil" => Op::Ceil,
            "abs" => Op::Abs,
            "saturate" => Op::Saturate,
            "round" => Op::Round,
            "sign" => Op::Sign,
            "truncate" => Op::Truncate,
            // (RSF round 20)
            "trig0" => Op::Trig(0),
            "trig1" => Op::Trig(1),
            "trig2" => Op::Trig(2),
            "trig3" => Op::Trig(3),
            "trig4" => Op::Trig(4),
            "trig5" => Op::Trig(5),
            "trig6" => Op::Trig(6),
            _ => Op::Bad,
        };
        Expr::Op(op, v.get("a").and_then(Value::as_array).into_iter().flatten().map(Expr::parse).collect())
    }

    /// Stock `FMaterialUniformExpression::GetNumberValue` of each class.
    pub fn eval(&self, scalars: &HashMap<String, f32>, vectors: &HashMap<String, [f32; 4]>, time: f32) -> [f32; 4] {
        match self {
            Expr::Const(c) => *c,
            Expr::Scalar(name, default) => [*scalars.get(name).unwrap_or(default); 4],
            Expr::Vector(name, default) => *vectors.get(name).unwrap_or(default),
            Expr::Time => [time; 4],
            Expr::Op(op, args) => {
                // (no operation takes more than three arguments; a missing one reads as zero)
                let mut a = [[0.0f32; 4]; 3];
                for (slot, x) in a.iter_mut().zip(args) {
                    *slot = x.eval(scalars, vectors, time);
                }
                let get = |k: usize| a[k];
                let one = |f: fn(f32) -> f32| get(0).map(f);
                let two = |f: fn(f32, f32) -> f32| {
                    let (x, y) = (get(0), get(1));
                    [f(x[0], y[0]), f(x[1], y[1]), f(x[2], y[2]), f(x[3], y[3])]
                };
                match op {
                    Op::Add => two(|x, y| x + y),
                    Op::Sub => two(|x, y| x - y),
                    Op::Mul => two(|x, y| x * y),
                    Op::Div => two(|x, y| if y.abs() < 1.0e-5 { 0.0 } else { x / y }),
                    Op::Dot(n) => {
                        let (x, y) = (get(0), get(1));
                        [(0..(*n).min(4)).map(|k| x[k] * y[k]).sum(); 4]
                    }
                    Op::Append(n) => {
                        let (x, y) = (get(0), get(1));
                        let mut out = [0.0; 4];
                        for (k, slot) in out.iter_mut().enumerate() {
                            *slot = if k < *n { x[k] } else { y[(k - n).min(3)] };
                        }
                        out
                    }
                    Op::Swizzle(index, n) => {
                        let x = get(0);
                        let mut out = [0.0; 4];
                        for k in 0..4 {
                            if k < *n && index[k] >= 0 {
                                out[k] = x[(index[k] as usize).min(3)];
                            }
                        }
                        out
                    }
                    Op::Clamp => {
                        let (x, lo, hi) = (get(0), get(1), get(2));
                        [x[0].max(lo[0]).min(hi[0]), x[1].max(lo[1]).min(hi[1]), x[2].max(lo[2]).min(hi[2]), x[3].max(lo[3]).min(hi[3])]
                    }
                    Op::Min => two(f32::min),
                    Op::Max => two(f32::max),
                    Op::Fmod => two(|x, y| if y == 0.0 { 0.0 } else { x % y }),
                    Op::Sin => one(f32::sin),
                    Op::Cos => one(f32::cos),
                    Op::Sqrt => one(|x| x.max(0.0).sqrt()),
                    Op::Frac => one(|x| x - x.floor()),
                    Op::Floor => one(f32::floor),
                    Op::Ceil => one(f32::ceil),
                    Op::Abs => one(f32::abs),
                    Op::Saturate => one(|x| x.clamp(0.0, 1.0)),
                    Op::Round => one(f32::round),
                    Op::Sign => one(|x| if x > 0.0 { 1.0 } else if x < 0.0 { -1.0 } else { 0.0 }),
                    Op::Truncate => one(f32::trunc),
                    // (RSF round 20) as the translator's evalTree: a non-finite result reads 0
                    Op::Trig(6) => two(|x, y| x.atan2(y)),
                    Op::Trig(k) => {
                        let f: fn(f32) -> f32 = match k {
                            0 => f32::sin,
                            1 => f32::cos,
                            2 => f32::tan,
                            3 => f32::asin,
                            4 => f32::acos,
                            _ => f32::atan,
                        };
                        get(0).map(|x| {
                            let v = f(x);
                            if v.is_finite() { v } else { 0.0 }
                        })
                    }
                    Op::Bad => [0.0; 4],
                }
            }
        }
    }

    pub fn uses_time(&self) -> bool {
        match self {
            Expr::Time => true,
            Expr::Op(_, args) => args.iter().any(Expr::uses_time),
            _ => false,
        }
    }
}

/// The constant buffer of a shader owner: vector rows, then the scalars packed four to a row.
#[derive(Debug, Default)]
pub struct OwnerCb {
    pub vec: Vec<Expr>,
    pub sca: Vec<Expr>,
    pub timed: bool,
    /// (IMPL_ER59) A RENUMBERED owner (an outfit's material that reads rows past MAX_CB, tools/sora_material_pack.js
    /// `cb_rows`): row k of the block the shader reads is row `cb_rows[k]` of the shader map's buffer. Empty = as is.
    pub rows: Vec<usize>,
}

impl OwnerCb {
    pub fn parse(owner: &Value) -> OwnerCb {
        let list = |key: &str| -> Vec<Expr> { owner.get(key).and_then(Value::as_array).into_iter().flatten().map(Expr::parse).collect() };
        let (vec, sca) = (list("vec"), list("sca"));
        let timed = vec.iter().chain(&sca).any(Expr::uses_time);
        let rows = owner.get("cb_rows").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_u64).map(|r| r as usize).collect();
        OwnerCb { vec, sca, timed, rows }
    }

    pub fn eval(&self, scalars: &HashMap<String, f32>, vectors: &HashMap<String, [f32; 4]>, time: f32, out: &mut [Vec4; MAX_CB]) {
        if !self.rows.is_empty() {
            // (IMPL_ER59) the whole buffer, then the rows the shader reads into their new places
            let mut full = vec![Vec4::ZERO; self.vec.len() + self.sca.len().div_ceil(4) + 1];
            for (k, e) in self.vec.iter().enumerate() {
                full[k] = Vec4::from_array(e.eval(scalars, vectors, time));
            }
            for (k, e) in self.sca.iter().enumerate() {
                full[self.vec.len() + k / 4][k % 4] = e.eval(scalars, vectors, time)[0];
            }
            for (k, row) in self.rows.iter().enumerate().take(MAX_CB) {
                out[k] = full.get(*row).copied().unwrap_or(Vec4::ZERO);
            }
            return;
        }
        for (k, e) in self.vec.iter().enumerate().take(MAX_CB) {
            out[k] = Vec4::from_array(e.eval(scalars, vectors, time));
        }
        for (k, e) in self.sca.iter().enumerate() {
            let row = self.vec.len() + k / 4;
            if row < MAX_CB {
                out[row][k % 4] = e.eval(scalars, vectors, time)[0];
            }
        }
    }
}

// ---------------------------------------------------------------------------- collections, KHS

/// A material parameter collection: its rows and where each parameter lies (component -1 = a
/// whole row).
#[derive(Clone, Debug, Default)]
pub struct Collection {
    pub rows: Vec<[f32; 4]>,
    pub params: HashMap<String, (usize, i32)>,
}

impl Collection {
    pub fn parse(v: &Value) -> Collection {
        let rows = v.get("rows").and_then(Value::as_array).into_iter().flatten().map(four).collect();
        let params = v.get("params").and_then(Value::as_object).into_iter().flatten().filter_map(|(n, p)| Some((n.clone(), (p.get("row")?.as_u64()? as usize, p.get("comp")?.as_i64()? as i32)))).collect();
        Collection { rows, params }
    }

    /// Writes a parameter (a number or four) into rows.
    pub fn set(&self, rows: &mut [Vec4], name: &str, value: &Value) -> bool {
        let Some(&(row, comp)) = self.params.get(name) else { return false };
        let Some(slot) = rows.get_mut(row) else { return false };
        match (comp, value.as_f64()) {
            (0..=3, Some(x)) => slot[comp as usize] = x as f32,
            (-1, None) => *slot = Vec4::from_array(four(value)),
            _ => return false,
        }
        true
    }

    pub fn rows<const N: usize>(&self) -> [Vec4; N] {
        let mut out = [Vec4::ZERO; N];
        for (k, row) in self.rows.iter().enumerate().take(N) {
            out[k] = Vec4::from_array(*row);
        }
        out
    }
}

/// The post-process material's data of the pack (`khs` of materials.json).
#[derive(Default)]
pub struct Khs {
    pub control: Collection,
    pub refkit: Collection,
    /// name -> (source text, control values, refkit values)
    pub presets: Vec<(String, String, Value, Value)>,
    /// The colour table's file and whether it is sRGB.
    pub table: Option<(String, bool)>,
}

/// The two collections as an instance's materials see them.
#[derive(Clone, Debug)]
pub struct KhsState {
    /// `false`: the post-process material is off (the lights are added to the base pass as is).
    pub on: bool,
    pub preset: String,
    pub c0: [Vec4; MAX_COLLECTION],
    pub k0: [Vec4; KHS_ROWS],
}

impl Khs {
    /// From materials.json (`None`-like default when the pack has no translated KHS).
    pub fn parse(json: &Value) -> Khs {
        let mut khs = Khs::default();
        if let Some(k) = json.get("khs").filter(|k| k.get("ok").and_then(Value::as_bool) == Some(true)) {
            let coll = |name: &str| k.get("collections").and_then(|c| c.get(name)).map(Collection::parse).unwrap_or_default();
            khs.control = coll("c0");
            khs.refkit = coll("k0");
            for (name, p) in k.get("presets").and_then(Value::as_object).into_iter().flatten() {
                khs.presets.push((name.clone(), p.get("source").and_then(Value::as_str).unwrap_or("").to_string(), p.get("control").cloned().unwrap_or_default(), p.get("refkit").cloned().unwrap_or_default()));
            }
            khs.table = k.get("textures").and_then(|t| t.get(0)).and_then(|t| Some((t.get("file")?.as_str()?.to_string(), t.get("srgb").and_then(Value::as_bool).unwrap_or(true))));
        }
        khs
    }

    /// The collections' defaults with a preset's values written over them (an unknown name: the
    /// defaults).
    pub fn preset_state(&self, preset: &str, on: bool) -> KhsState {
        let mut state = KhsState { on, preset: preset.to_string(), c0: self.control.rows(), k0: self.refkit.rows() };
        if let Some((_, _, control, refkit)) = self.presets.iter().find(|(n, ..)| n == preset) {
            for (name, value) in control.as_object().into_iter().flatten() {
                self.control.set(&mut state.c0, name, value);
            }
            for (name, value) in refkit.as_object().into_iter().flatten() {
                self.refkit.set(&mut state.k0, name, value);
            }
        }
        state
    }
}

// ---------------------------------------------------------------------------- bone parameters

/// A row of the mesh asset's `m_BonePoseToMaterialParam`.
#[derive(Clone, Debug)]
pub struct BoneParam {
    pub bone: String,
    /// `ESqEX_BonePoseToMaterialCheckType` without its prefix.
    pub check: String,
    pub parameter: String,
    pub vector: bool,
    /// The material instances it is written on (empty = all).
    pub materials: Vec<String>,
    pub normalize: bool,
    pub normalize_min: Vec3,
    pub normalize_max: Vec3,
    pub set_min: Vec3,
    pub set_max: Vec3,
}

impl BoneParam {
    /// The rows of a mesh's `bone_params` (materials.json `mesh_facts.<mesh>`).
    pub fn parse_table(facts: &Value) -> Vec<BoneParam> {
        let v3 = |v: &Value| Vec3::new(v[0].as_f64().unwrap_or(0.0) as f32, v[1].as_f64().unwrap_or(0.0) as f32, v[2].as_f64().unwrap_or(0.0) as f32);
        let mut table = Vec::new();
        for b in facts.get("bone_params").and_then(Value::as_array).into_iter().flatten() {
            let text = |k: &str| b.get(k).and_then(Value::as_str).unwrap_or("").to_string();
            table.push(BoneParam { bone: text("bone"), check: text("check"), parameter: text("parameter"), vector: text("set") == "VECTOR", materials: b.get("materials").and_then(Value::as_array).into_iter().flatten().filter_map(|m| m.as_str().map(str::to_string)).collect(), normalize: b.get("normalize").and_then(Value::as_bool).unwrap_or(false), normalize_min: v3(&b["normalize_min"]), normalize_max: v3(&b["normalize_max"]), set_min: v3(&b["set_min"]), set_max: v3(&b["set_max"]) });
        }
        table
    }

    /// The value for a bone whose pose is `local` (to its parent) and `world`. `None`: a check
    /// type this port does not evaluate (the rotation and difference types; Sora's table has
    /// none). [D] The remap is the natural reading of the four range members (per component:
    /// the normalise range onto 0..1, clamped, then onto the set range); the engine's evaluator
    /// was not traced.
    pub fn value(&self, local: &kh_anim::Trs, component: &kh_anim::Trs, world: &kh_anim::Trs) -> Option<[f32; 4]> {
        let v = match self.check.as_str() {
            "LOCAL_ABS_TRANS" => local.translation,
            "COMPONENT_ABS_TRANS" => component.translation,
            "WORLD_ABS_TRANS" => world.translation,
            "WORLD_ABS_MAT_X" => world.rotation * Vec3::X * world.scale.x,
            "WORLD_ABS_MAT_Y" => world.rotation * Vec3::Y * world.scale.y,
            "WORLD_ABS_MAT_Z" => world.rotation * Vec3::Z * world.scale.z,
            "WORLD_ABS_SCALE" => world.scale,
            _ => return None,
        };
        let v = if self.normalize {
            let one = |x: f32, a: f32, b: f32, lo: f32, hi: f32| if (b - a).abs() < 1.0e-6 { lo } else { lo + ((x - a) / (b - a)).clamp(0.0, 1.0) * (hi - lo) };
            Vec3::new(one(v.x, self.normalize_min.x, self.normalize_max.x, self.set_min.x, self.set_max.x), one(v.y, self.normalize_min.y, self.normalize_max.y, self.set_min.y, self.set_max.y), one(v.z, self.normalize_min.z, self.normalize_max.z, self.set_min.z, self.set_max.z))
        } else {
            v
        };
        Some(v.extend(1.0).to_array())
    }
}

/// A bone-driven parameter of a frame: name, value, the material instances it goes to (empty = all).
pub type BoneValue = (String, [f32; 4], Vec<String>);

/// The mesh asset's table evaluated for a pose: `bone` gives a bone's local (to its parent),
/// component and world transform (game space, cm).
pub fn bone_parameters(table: &[BoneParam], bone: impl Fn(&str) -> Option<(kh_anim::Trs, kh_anim::Trs, kh_anim::Trs)>) -> Vec<BoneValue> {
    table
        .iter()
        .filter_map(|row| {
            let (local, component, world) = bone(&row.bone)?;
            Some((row.parameter.clone(), row.value(&local, &component, &world)?, row.materials.clone()))
        })
        .collect()
}

// ---------------------------------------------------------------------------- rig and environment

/// A light rig: the key light and the indirect lighting cache.
#[derive(Clone, Debug)]
pub struct Rig {
    /// Empty: the key light is the level's directional light (its direction and shadow).
    /// Otherwise the rig's own spot lights (at most two) light the model and `key` tints them.
    pub spots: Vec<Spot>,
    /// Colour x intensity, engine units; with `key_of_level` a scale of the level's
    /// directional light as the camera exposes it.
    pub key: Vec3,
    pub key_of_level: bool,
    /// The indirect term's constant band and vertical gradient (0 = flat); with
    /// `ambient_of_level` a scale of the level's ambient light as the camera exposes it.
    pub ambient: Vec3,
    pub ambient_of_level: bool,
    pub sky_gradient: f32,
    pub shadows: bool,
    /// What a reflection sees.
    pub env: Env,
    /// (game over) Point lights of the rig (at most four; a rig with spots or points is lit by them alone):
    /// the particle lights of an effect (`ParticleModuleLight`, exponent falloff).
    pub points: Vec<PointLight>,
}

/// (game over) A point light with the engine's exponent falloff (`bUseInverseSquaredFalloff` off: the
/// radial attenuation is `pow(1 - saturate(d^2 / r^2), exponent)`), as a `ParticleModuleLight` particle gives it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PointLight {
    /// Game space, cm.
    pub position: Vec3,
    /// Colour x brightness (`ColorScaleOverLife` x `BrightnessOverLife` x the particle's colour).
    pub color: Vec3,
    /// `RadiusScale` x the mean of the particle's X and Y size, cm.
    pub radius: f32,
    /// `LightExponent`.
    pub exponent: f32,
}

/// The environment a rig's reflections look up, as a small prefiltered cube (GENERATED: the
/// game's reflection captures and sky light cube are not in the pack).
#[derive(Clone, Debug)]
pub enum Env {
    /// Sky overhead, sky at the horizon, ground (the test level).
    Sky { zenith: Vec3, horizon: Vec3, ground: Vec3 },
    /// A cube map known by the mean colour of each face, game axes +X -X +Y -Y +Z -Z (the camp
    /// menu's ambient cube map).
    Faces([Vec3; 6]),
}

pub const ENV_SIZE: u32 = 32;

impl Env {
    /// The colour in the direction `d` (game space, unit) for a surface of the given roughness.
    pub fn sample(&self, d: Vec3, roughness: f32) -> Vec3 {
        // The lobe a rough surface averages over, as a blur of the picture.
        let blur = (roughness * roughness).clamp(0.0, 1.0);
        match self {
            Env::Sky { zenith, horizon, ground } => {
                let width = 0.04 + 0.9 * blur;
                let up = ((d.z / width) * 0.5 + 0.5).clamp(0.0, 1.0);
                let up = up * up * (3.0 - 2.0 * up);
                let sky = horizon.lerp(*zenith, d.z.max(0.0).powf(0.6 + blur));
                ground.lerp(sky, up)
            }
            Env::Faces(faces) => {
                let mean = faces.iter().sum::<Vec3>() / 6.0;
                let w = [d.x.max(0.0), (-d.x).max(0.0), d.y.max(0.0), (-d.y).max(0.0), d.z.max(0.0), (-d.z).max(0.0)];
                let sharp: Vec3 = (0..6).map(|k| faces[k] * w[k] * w[k]).sum();
                sharp.lerp(mean, blur)
            }
        }
    }

    /// The cube's texels: `ENV_SIZE` a side, RGBA half floats, face after face (+X -X +Y -Y +Z -Z
    /// in the layout of a Y-up host: the shader looks up (x, z, y) of a game direction), each
    /// face's mips one after the other; every mip the picture for the roughness the engine's
    /// look-up reads that mip for (`ComputeReflectionCaptureMipFromRoughness` inverted).
    /// Returns the bytes and the mip count.
    pub fn cube_rgba16f(&self) -> (Vec<u8>, u32) {
        let mips = ENV_SIZE.trailing_zeros() + 1;
        let mut data: Vec<u8> = Vec::new();
        for face in 0..6 {
            for mip in 0..mips {
                let n = (ENV_SIZE >> mip).max(1);
                let level_from_1x1 = (mips - 1) as f32 - 1.0 - mip as f32;
                let roughness = (2.0f32).powf((1.0 - level_from_1x1) / 1.2).clamp(0.0, 1.0);
                for y in 0..n {
                    for x in 0..n {
                        let (u, v) = ((x as f32 + 0.5) / n as f32 * 2.0 - 1.0, (y as f32 + 0.5) / n as f32 * 2.0 - 1.0);
                        // The cube's faces in Bevy's axes; the picture is defined in the game's.
                        let b = match face {
                            0 => Vec3::new(1.0, -v, -u),
                            1 => Vec3::new(-1.0, -v, u),
                            2 => Vec3::new(u, 1.0, v),
                            3 => Vec3::new(u, -1.0, -v),
                            4 => Vec3::new(u, -v, 1.0),
                            _ => Vec3::new(-u, -v, -1.0),
                        }
                        .normalize();
                        let c = self.sample(Vec3::new(b.x, b.z, b.y), roughness);
                        for value in [c.x, c.y, c.z, 1.0] {
                            data.extend_from_slice(&half(value).to_le_bytes());
                        }
                    }
                }
            }
        }
        (data, mips)
    }
}

/// An f32 as IEEE half bits (finite, non-negative values of a colour).
pub fn half(value: f32) -> u16 {
    let v = value.clamp(0.0, 65000.0);
    if v < 6.1e-5 {
        return 0;
    }
    let bits = v.to_bits();
    let exponent = ((bits >> 23) & 0xFF) as i32 - 127 + 15;
    let mantissa = (bits >> 13) & 0x3FF;
    ((exponent.clamp(1, 30) as u16) << 10) | mantissa as u16
}

/// A spot light of a rig (stock UE 4.17 `USpotLightComponent`, inverse-squared falloff).
#[derive(Clone, Copy, Debug)]
pub struct Spot {
    /// Game space, cm.
    pub position: Vec3,
    /// The axis it points along (unit).
    pub direction: Vec3,
    /// The component's Intensity.
    pub intensity: f32,
    pub inner_cone_deg: f32,
    pub outer_cone_deg: f32,
    pub radius: f32,
    pub min_roughness: f32,
}

impl Rig {
    /// The indirect lighting cache's SH rows (PrecomputedLightingBuffer rows 8..15).
    pub fn sh(&self) -> [Vec4; 8] {
        // DotSH3 against CalcDiffuseTransferSH3(normal): band 0 weighs 0.886228, the z term of
        // band 1 is 1.023328 * normal.z (PrecomputedLightingBuffer rows 8..10 = SH0 R, G, B).
        let mut sh = [Vec4::ZERO; 8];
        for c in 0..3 {
            sh[c] = Vec4::new(self.ambient[c] / 0.886_228, 0.0, self.ambient[c] * self.sky_gradient / 1.023_328, 0.0);
        }
        sh
    }

    /// The spot rows of the uniform block (position + intensity x 16, axis + radius, cone).
    pub fn spot_rows(&self) -> ([Vec4; 2], [Vec4; 2], [Vec4; 2]) {
        let (mut pos, mut dir, mut cone) = ([Vec4::ZERO; 2], [Vec4::Z; 2], [Vec4::ZERO; 2]);
        for k in 0..2 {
            if let Some(s) = self.spots.get(k) {
                // A point light's colour is its Intensity x 16 with inverse-squared falloff (stock 4.17).
                let (inner, outer) = (s.inner_cone_deg.to_radians().cos(), s.outer_cone_deg.to_radians().cos());
                pos[k] = s.position.extend(s.intensity * 16.0);
                dir[k] = s.direction.normalize_or_zero().extend(s.radius);
                cone[k] = Vec4::new(outer, 1.0 / (inner - outer).max(1.0e-4), s.min_roughness, 0.0);
            }
        }
        (pos, dir, cone)
    }

    /// (game over) The point rows of the uniform block (position + radius, colour + exponent).
    pub fn point_rows(&self) -> ([Vec4; 4], [Vec4; 4]) {
        let (mut pos, mut col) = ([Vec4::ZERO; 4], [Vec4::ZERO; 4]);
        for (k, p) in self.points.iter().take(4).enumerate() {
            pos[k] = p.position.extend(p.radius);
            col[k] = p.color.extend(p.exponent);
        }
        (pos, col)
    }

    /// (game over) The rig has lights of its own (the level's key light is not used).
    pub fn own_lights(&self) -> bool {
        !self.spots.is_empty() || !self.points.is_empty()
    }
}

// ---------------------------------------------------------------------------- the uniform block

/// `KhParams` of the harness as plain rows (every member is a `vec4<f32>` or an array of them, so
/// the layout is the same under every packing rule): for a host that uploads bytes itself. The
/// Bevy host has its own mirror with the same member order.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Params {
    pub m: [Vec4; MAX_CB],
    pub c0: [Vec4; MAX_COLLECTION],
    pub k0: [Vec4; KHS_ROWS],
    pub w2l: [Vec4; 4],
    pub obj: Vec4,
    pub bounds: Vec4,
    pub actor: Vec4,
    pub bmin: Vec4,
    pub bmax: Vec4,
    pub sh: [Vec4; 8],
    pub light_dir: Vec4,
    pub light_color: Vec4,
    pub misc: Vec4,
    pub look: Vec4,
    pub khs: Vec4,
    pub env: Vec4,
    pub spot_pos: [Vec4; 2],
    pub spot_dir: [Vec4; 2],
    pub spot_cone: [Vec4; 2],
    /// (game over) Point lights: xyz position (game space, cm), w the radius (0 = none); rgb colour x
    /// brightness, w the falloff exponent. APPENDED: a shader built before them reads the block as before.
    pub point_pos: [Vec4; 4],
    pub point_col: [Vec4; 4],
    /// (NF) KH3's camera fade of his character: x = 1 - the CameraFade value (0 = whole), y = the screen door's phase
    /// (char_common.wgsl `kh_camfade_*`, kh-camera camfade.rs). APPENDED, as the point lights.
    pub fade: Vec4,
}

/// Rows of [`Params`].
pub const PARAM_ROWS: usize = MAX_CB + MAX_COLLECTION + KHS_ROWS + 4 + 5 + 8 + 6 + 6 + 8 + 1; // (NF) + fade

impl Default for Params {
    fn default() -> Params {
        Params { m: [Vec4::ZERO; MAX_CB], c0: [Vec4::ZERO; MAX_COLLECTION], k0: [Vec4::ZERO; KHS_ROWS], w2l: [Vec4::X, Vec4::Y, Vec4::Z, Vec4::ZERO], obj: Vec4::ZERO, bounds: Vec4::ONE, actor: Vec4::ZERO, bmin: Vec4::ZERO, bmax: Vec4::ONE, sh: [Vec4::ZERO; 8], light_dir: Vec4::new(0.0, 0.0, 1.0, 0.0), light_color: Vec4::ONE, misc: Vec4::new(0.0, 0.0, 1.0, 1.0), look: Vec4::ONE, khs: Vec4::ZERO, env: Vec4::new(0.0, 1.0, 0.0, 0.0), spot_pos: [Vec4::ZERO; 2], spot_dir: [Vec4::Z; 2], spot_cone: [Vec4::ZERO; 2], point_pos: [Vec4::ZERO; 4], point_col: [Vec4::ZERO; 4], fade: Vec4::ZERO /* (NF) */ }
    }
}

impl Params {
    /// The block as the floats the shader reads, in member order, without copying (the struct is
    /// rows of four floats and nothing else: the test below holds it to that).
    pub fn as_floats(&self) -> &[f32; PARAM_ROWS * 4] {
        // SAFETY: `Params` is `repr(C)`, made of `Vec4` only (16 bytes, four f32 each), its size is
        // PARAM_ROWS * 16 (asserted in the tests) and its alignment is at least an f32's.
        unsafe { &*(self as *const Params as *const [f32; PARAM_ROWS * 4]) }
    }

    /// The block as the floats the shader reads, in member order.
    pub fn floats(&self) -> Vec<f32> {
        let mut out = Vec::with_capacity(PARAM_ROWS * 4);
        let mut push = |v: &Vec4| out.extend_from_slice(&v.to_array());
        self.m.iter().for_each(&mut push);
        self.c0.iter().for_each(&mut push);
        self.k0.iter().for_each(&mut push);
        self.w2l.iter().for_each(&mut push);
        for v in [&self.obj, &self.bounds, &self.actor, &self.bmin, &self.bmax] {
            push(v);
        }
        self.sh.iter().for_each(&mut push);
        for v in [&self.light_dir, &self.light_color, &self.misc, &self.look, &self.khs, &self.env] {
            push(v);
        }
        self.spot_pos.iter().for_each(&mut push);
        self.spot_dir.iter().for_each(&mut push);
        self.spot_cone.iter().for_each(&mut push);
        self.point_pos.iter().for_each(&mut push);
        self.point_col.iter().for_each(&mut push);
        push(&self.fade); // (NF)
        out
    }
}

/// STAND-IN, IN ONE PLACE: the lights a host without a level of its own gives Sora (the Elden
/// Ring renderer, until the game's sun, ambient light and exposure are read). They are the Bevy
/// test level's lights in the engine's units, so the character looks as he does in `khgame.exe`:
/// the sun 9000 lx and the ambient light 400 cd/m2 under the camera's exposure 1 / 998
/// (IMPL_CHARMAT_STATE.md, third pass), the sun's direction of kh-app/src/level.rs, the test
/// level's sky as the reflection environment, the camera's filmic curve (ACES fit) and sRGB.
pub mod stand_in {
    use glam::Vec3;

    /// Colour x intensity of the key light: a white Lambert surface facing it shows this / pi.
    pub const KEY: f32 = 9000.0 / 998.0;
    /// The indirect term's constant band.
    pub const AMBIENT: f32 = 400.0 / 998.0;
    /// Unit vector TO the key light, game space (X forward, Y right, Z up of the KH3 frame the
    /// host anchors where Sora started): kh-app's `looking_to(0.45, -1, 0.35)` in game axes.
    pub const KEY_DIR: Vec3 = Vec3::new(-0.45, -0.35, 1.0);
    pub const ENV_ZENITH: Vec3 = Vec3::new(0.24, 0.40, 0.68);
    pub const ENV_HORIZON: Vec3 = Vec3::new(0.52, 0.62, 0.74);
    pub const ENV_GROUND: Vec3 = Vec3::new(0.26, 0.26, 0.27);
    /// The exposure in front of the tone curve.
    pub const EXPOSURE: f32 = 1.0;

    pub fn rig() -> super::Rig {
        super::Rig { spots: Vec::new(), key: Vec3::splat(KEY), key_of_level: false, ambient: Vec3::splat(AMBIENT), ambient_of_level: false, sky_gradient: 0.0, shadows: false, env: super::Env::Sky { zenith: ENV_ZENITH, horizon: ENV_HORIZON, ground: ENV_GROUND }, points: Vec::new() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// (RSF round 20) TrigMath in a time-dependent tree: the UV turn of the two-layer masters (cos / sin of a rotation)
    #[test]
    fn rsf20_trig_math_is_evaluated() {
        let (s, v) = (HashMap::new(), HashMap::new());
        let tree = |f: &str| Expr::parse(&serde_json::json!({ "f": f, "a": [{ "c": [0.0, 0.5, 1.0, 0.0] }, { "c": [1.0, 1.0, 1.0, 1.0] }] }));
        assert_eq!(tree("trig1").eval(&s, &v, 0.0)[0], 1.0);
        assert!((tree("trig0").eval(&s, &v, 0.0)[1] - 0.5f32.sin()).abs() < 1e-6);
        assert!((tree("trig6").eval(&s, &v, 0.0)[2] - 1.0f32.atan2(1.0)).abs() < 1e-6);
        assert_eq!(tree("trig3").eval(&s, &v, 0.0)[0], 0.0);
    }

    #[test]
    fn uniform_expressions_follow_the_engine() {
        let (mut s, mut v) = (HashMap::new(), HashMap::new());
        s.insert("Speed".to_string(), 2.0);
        v.insert("Tint".to_string(), [0.5, 0.25, 1.0, 1.0]);
        let e = |text: &str| Expr::parse(&serde_json::from_str::<Value>(text).unwrap()).eval(&s, &v, 3.0);
        assert_eq!(e(r#"{"s":"Speed","d":7}"#), [2.0; 4]);
        assert_eq!(e(r#"{"s":"Other","d":7}"#), [7.0; 4]);
        assert_eq!(e(r#"{"f":"mul","n":4,"a":[{"t":1},{"s":"Speed","d":0}]}"#), [6.0; 4]);
        assert_eq!(e(r#"{"f":"append","n":3,"a":[{"v":"Tint","d":[0,0,0,0]},{"c":[9,9,9,9]}]}"#), [0.5, 0.25, 1.0, 9.0]);
        assert_eq!(e(r#"{"f":"swizzle","i":[2,0,-1,-1],"n":2,"a":[{"v":"Tint","d":[0,0,0,0]}]}"#), [1.0, 0.5, 0.0, 0.0]);
        assert_eq!(e(r#"{"f":"dot","n":3,"a":[{"v":"Tint","d":[0,0,0,0]},{"c":[1,1,1,1]}]}"#), [1.75; 4]);
        // Division by (nearly) zero and fmod by zero give 0, not NaN.
        assert_eq!(e(r#"{"f":"div","n":4,"a":[{"c":[1,1,1,1]},{"c":[0,0,0,0]}]}"#), [0.0; 4]);
        let cb = OwnerCb::parse(&serde_json::from_str(r#"{"vec":[{"v":"Tint","d":[0,0,0,0]}],"sca":[{"c":[1,1,1,1]},{"s":"Speed","d":0},{"c":[3,3,3,3]},{"c":[4,4,4,4]},{"c":[5,5,5,5]}]}"#).unwrap());
        let mut out = [Vec4::ZERO; MAX_CB];
        cb.eval(&s, &v, 0.0, &mut out);
        assert_eq!((out[0], out[1], out[2].x), (Vec4::new(0.5, 0.25, 1.0, 1.0), Vec4::new(1.0, 2.0, 3.0, 4.0), 5.0));
        assert!(!cb.timed);
    }

    #[test]
    fn the_uniform_block_is_rows_of_four() {
        // The shader's struct is vec4 rows only: the plain mirror must have no padding and the
        // float list must cover it exactly, in member order.
        assert_eq!(std::mem::size_of::<Params>(), PARAM_ROWS * 16);
        let mut p = Params::default();
        p.m[1] = Vec4::new(1.0, 2.0, 3.0, 4.0);
        p.point_col[3].w = 9.0;
        p.sh[0].x = 7.0;
        let f = p.floats();
        assert_eq!(f.len(), PARAM_ROWS * 4);
        assert_eq!(f.as_slice(), p.as_floats().as_slice());
        assert_eq!(&f[4..8], &[1.0, 2.0, 3.0, 4.0]);
        // (NF) the camera fade's row follows the point lights
        assert_eq!(f[f.len() - 5], 9.0);
        p.fade.x = 0.5;
        assert_eq!(p.floats()[PARAM_ROWS * 4 - 4], 0.5);
        assert_eq!(f[(MAX_CB + MAX_COLLECTION + KHS_ROWS + 4 + 5) * 4], 7.0);
    }

    #[test]
    fn the_stand_in_rig_is_the_test_levels_light() {
        let rig = stand_in::rig();
        let sh = rig.sh();
        // The indirect bands evaluate to the ambient value for any normal.
        let eval = |nz: f32| sh[0].x * 0.886_228 + sh[0].z * 1.023_328 * nz;
        assert!((eval(1.0) - stand_in::AMBIENT).abs() < 1.0e-5 && (eval(-1.0) - stand_in::AMBIENT).abs() < 1.0e-5);
        assert!(!rig.key_of_level && !rig.ambient_of_level && (rig.key.x - 9.018).abs() < 0.01);
    }
}

#[cfg(test)]
mod er59_tests {
    use super::*;

    /// (IMPL_ER59) A renumbered owner: the shader reads rows 0 and 170 of the buffer as its rows 0 and 1.
    #[test]
    fn a_renumbered_owner_puts_the_rows_it_reads_first() {
        let vec: Vec<serde_json::Value> = (0..171).map(|k| serde_json::json!({"c": [k as f32, 0, 0, 0]})).collect();
        let owner = serde_json::json!({"vec": vec, "sca": [], "cb_rows": [0, 170]});
        let cb = OwnerCb::parse(&owner);
        let mut out = [Vec4::splat(-1.0); MAX_CB];
        cb.eval(&HashMap::new(), &HashMap::new(), 0.0, &mut out);
        assert_eq!((out[0].x, out[1].x, out[2].x), (0.0, 170.0, -1.0));
    }
}
