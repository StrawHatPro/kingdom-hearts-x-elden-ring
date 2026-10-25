//! The AI pack (`tools/party_ai_pack.js` -> `build/sora_pack/party/ai/`), parsed.
//!
//! The host hands kh-ai the text of each file (no file access here). Trees keep their node
//! properties as JSON maps: each node class reads what it needs when the tree is compiled
//! (`bt::compile`), with the class defaults the node module holds.

use serde_json::{Map, Value};
use std::collections::BTreeMap;

pub type Props = Map<String, Value>;

/// A decorator or service as packed: `{class, name, p}`.
#[derive(Clone, Debug)]
pub struct AuxDef {
    pub class: String,
    pub name: String,
    pub p: Props,
}

/// One child slot of a composite: its decorators, their logic ops and the child node.
#[derive(Clone, Debug)]
pub struct ChildDef {
    pub decorators: Vec<AuxDef>,
    /// `BTDecoratorLogic` ops (empty = AND of all decorators).
    pub ops: Vec<(String, u16)>,
    pub node: NodeDef,
}

#[derive(Clone, Debug)]
pub struct NodeDef {
    pub class: String,
    pub name: String,
    pub p: Props,
    pub services: Vec<AuxDef>,
    pub children: Vec<ChildDef>,
}

#[derive(Clone, Debug)]
pub struct TreeDef {
    pub name: String,
    pub path: String,
    pub blackboard: Option<String>,
    pub root_decorators: Vec<AuxDef>,
    pub root_ops: Vec<(String, u16)>,
    pub root: Option<NodeDef>,
}

#[derive(Clone, Debug)]
pub struct BbKeyDef {
    pub name: String,
    /// `BlackboardKeyType_*` without the prefix: Bool, Int, Float, Enum, NativeEnum, Name,
    /// String, Vector, Rotator, Object, Class.
    pub ty: String,
    pub p: Props,
    pub instance_synced: bool,
}

#[derive(Clone, Debug)]
pub struct BlackboardDef {
    pub name: String,
    pub parent: Option<String>,
    pub keys: Vec<BbKeyDef>,
}

#[derive(Clone, Debug)]
pub struct EqsOptionDef {
    pub generator: Option<AuxDef>,
    pub tests: Vec<AuxDef>,
}

#[derive(Clone, Debug)]
pub struct EqsDef {
    pub name: String,
    pub options: Vec<EqsOptionDef>,
}

/// A blueprint class the trees name (action / locomotion definitions, controller, contexts).
#[derive(Clone, Debug)]
pub struct ClassDef {
    pub name: String,
    pub native: Option<String>,
    pub chain: Vec<String>,
    pub p: Props,
}

#[derive(Clone, Debug)]
pub struct TableDef {
    pub name: String,
    pub row_struct: Option<String>,
    /// Rows in file order.
    pub rows: Vec<(String, Props)>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CurveKey {
    pub t: f32,
    pub v: f32,
    /// ERichCurveInterpMode: 0 linear, 1 constant, 2 cubic.
    pub interp: u8,
}

#[derive(Clone, Debug)]
pub struct CurveDef {
    pub name: String,
    /// One curve for a CurveFloat, three for a CurveVector.
    pub curves: Vec<Vec<CurveKey>>,
}

impl CurveDef {
    /// Evaluate curve `i` at `t` (constant / linear keys; cubic keys are treated as linear: the
    /// coordinator curves use 0 / 1 only [A]).
    pub fn eval(&self, i: usize, t: f32) -> f32 {
        let keys = match self.curves.get(i) {
            Some(k) if !k.is_empty() => k,
            _ => return 0.0,
        };
        if t <= keys[0].t {
            return keys[0].v;
        }
        for w in keys.windows(2) {
            let (a, b) = (w[0], w[1]);
            if t < b.t {
                return match a.interp {
                    1 => a.v,
                    _ => a.v + (b.v - a.v) * ((t - a.t) / (b.t - a.t)),
                };
            }
        }
        keys[keys.len() - 1].v
    }
}

/// Per-pawn roots (`index.json` "roots").
#[derive(Clone, Debug, Default)]
pub struct PawnRoots {
    pub tree: String,
    pub dynamic: Vec<(String, String)>,
    pub follow_query: Option<String>,
    pub locomotion: Option<String>,
    pub walk: Option<String>,
}

#[derive(Debug, Default)]
pub struct AiPack {
    pub trees: BTreeMap<String, TreeDef>,
    pub blackboards: BTreeMap<String, BlackboardDef>,
    pub eqs: BTreeMap<String, EqsDef>,
    pub classes: BTreeMap<String, ClassDef>,
    pub tables: BTreeMap<String, TableDef>,
    pub curves: BTreeMap<String, CurveDef>,
    pub data: BTreeMap<String, Props>,
    pub roots: BTreeMap<String, PawnRoots>,
    /// Instances per node class over the packed trees / queries (`index.json` "census").
    pub census: BTreeMap<String, u32>,
    /// Enum tables (`index.json` "enums"): enum -> value name (without the prefix) -> value.
    pub enums: BTreeMap<String, BTreeMap<String, i64>>,
}

fn s(v: &Value) -> String {
    v.as_str().unwrap_or("").to_string()
}

fn aux(v: &Value) -> AuxDef {
    AuxDef {
        class: s(&v["class"]),
        name: s(&v["name"]),
        p: v["p"].as_object().cloned().unwrap_or_default(),
    }
}

fn ops(v: &Value) -> Vec<(String, u16)> {
    v.as_array().map(|a| a.iter().map(|o| (s(&o["op"]), o["n"].as_u64().unwrap_or(0) as u16)).collect()).unwrap_or_default()
}

fn node(v: &Value) -> NodeDef {
    NodeDef {
        class: s(&v["class"]),
        name: s(&v["name"]),
        p: v["p"].as_object().cloned().unwrap_or_default(),
        services: v["services"].as_array().map(|a| a.iter().map(aux).collect()).unwrap_or_default(),
        children: v["children"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter(|c| !c["node"].is_null())
                    .map(|c| ChildDef {
                        decorators: c["decorators"].as_array().map(|d| d.iter().map(aux).collect()).unwrap_or_default(),
                        ops: ops(&c["ops"]),
                        node: node(&c["node"]),
                    })
                    .collect()
            })
            .unwrap_or_default(),
    }
}

/// The asset name of an object path ("/Game/A/B.B_C" -> "B").
pub fn asset_name(path: &str) -> &str {
    let last = path.rsplit('/').next().unwrap_or(path);
    let base = last.split('.').next().unwrap_or(last);
    base
}

impl AiPack {
    /// Parse the pack. `read(rel)` returns the text of `build/sora_pack/party/ai/<rel>`.
    pub fn load(read: &mut dyn FnMut(&str) -> Option<String>) -> Result<AiPack, String> {
        let index_text = read("index.json").ok_or("ai pack: index.json missing")?;
        let index: Value = serde_json::from_str(&index_text).map_err(|e| format!("ai pack index: {e}"))?;
        let mut pack = AiPack::default();
        if let Some(c) = index["census"].as_object() {
            for (k, v) in c {
                pack.census.insert(k.clone(), v.as_u64().unwrap_or(0) as u32);
            }
        }
        if let Some(e) = index["enums"].as_object() {
            for (en, vals) in e {
                let m = vals.as_object().map(|o| o.iter().map(|(k, v)| (k.clone(), v.as_i64().unwrap_or(-1))).collect()).unwrap_or_default();
                pack.enums.insert(en.clone(), m);
            }
        }
        if let Some(r) = index["roots"].as_object() {
            for (pawn, v) in r {
                pack.roots.insert(
                    pawn.clone(),
                    PawnRoots {
                        tree: s(&v["tree"]),
                        dynamic: v["dynamic"].as_array().map(|a| a.iter().map(|d| (s(&d["tag"]), s(&d["tree"]))).collect()).unwrap_or_default(),
                        follow_query: v["follow_query"].as_str().map(String::from),
                        locomotion: v["locomotion"].as_str().map(String::from),
                        walk: v["walk"].as_str().map(String::from),
                    },
                );
            }
        }
        let assets = index["assets"].as_object().ok_or("ai pack: no assets")?;
        for (name, a) in assets {
            let file = s(&a["file"]);
            let text = read(&file).ok_or_else(|| format!("ai pack: {file} missing"))?;
            let v: Value = serde_json::from_str(&text).map_err(|e| format!("ai pack {file}: {e}"))?;
            match a["kind"].as_str().unwrap_or("") {
                "tree" => {
                    pack.trees.insert(
                        name.clone(),
                        TreeDef {
                            name: name.clone(),
                            path: s(&v["path"]),
                            blackboard: v["blackboard"].as_str().map(String::from),
                            root_decorators: v["root_decorators"].as_array().map(|d| d.iter().map(aux).collect()).unwrap_or_default(),
                            root_ops: ops(&v["root_ops"]),
                            root: if v["root"].is_null() { None } else { Some(node(&v["root"])) },
                        },
                    );
                }
                "blackboard" => {
                    pack.blackboards.insert(
                        name.clone(),
                        BlackboardDef {
                            name: name.clone(),
                            parent: v["parent"].as_str().map(String::from),
                            keys: v["keys"]
                                .as_array()
                                .map(|k| {
                                    k.iter()
                                        .map(|k| BbKeyDef {
                                            name: s(&k["name"]),
                                            ty: s(&k["type"]),
                                            p: k["p"].as_object().cloned().unwrap_or_default(),
                                            instance_synced: k["instance_synced"].as_bool().unwrap_or(false),
                                        })
                                        .collect()
                                })
                                .unwrap_or_default(),
                        },
                    );
                }
                "eqs" => {
                    pack.eqs.insert(
                        name.clone(),
                        EqsDef {
                            name: name.clone(),
                            options: v["options"]
                                .as_array()
                                .map(|o| {
                                    o.iter()
                                        .map(|o| EqsOptionDef {
                                            generator: if o["generator"].is_null() { None } else { Some(aux(&o["generator"])) },
                                            tests: o["tests"].as_array().map(|t| t.iter().map(aux).collect()).unwrap_or_default(),
                                        })
                                        .collect()
                                })
                                .unwrap_or_default(),
                        },
                    );
                }
                "class" => {
                    pack.classes.insert(
                        name.clone(),
                        ClassDef {
                            name: name.clone(),
                            native: v["native"].as_str().map(String::from),
                            chain: v["chain"].as_array().map(|c| c.iter().map(s).collect()).unwrap_or_default(),
                            p: v["p"].as_object().cloned().unwrap_or_default(),
                        },
                    );
                }
                "table" => {
                    let rows = v["rows"].as_object().map(|r| r.iter().map(|(k, v)| (k.clone(), v.as_object().cloned().unwrap_or_default())).collect()).unwrap_or_default();
                    pack.tables.insert(name.clone(), TableDef { name: name.clone(), row_struct: v["row_struct"].as_str().map(String::from), rows });
                }
                "curve" => {
                    let curves = v["curves"]
                        .as_array()
                        .map(|cs| {
                            cs.iter()
                                .map(|c| {
                                    c.as_array()
                                        .map(|k| {
                                            k.iter()
                                                .map(|k| CurveKey {
                                                    t: k["t"].as_f64().unwrap_or(0.0) as f32,
                                                    v: k["v"].as_f64().unwrap_or(0.0) as f32,
                                                    interp: k["interp"].as_u64().unwrap_or(0) as u8,
                                                })
                                                .collect()
                                        })
                                        .unwrap_or_default()
                                })
                                .collect()
                        })
                        .unwrap_or_default();
                    pack.curves.insert(name.clone(), CurveDef { name: name.clone(), curves });
                }
                "data" => {
                    pack.data.insert(name.clone(), v["p"].as_object().cloned().unwrap_or_default());
                }
                _ => {}
            }
        }
        Ok(pack)
    }

    /// Load from a directory on disk (tests and tools only; the crate itself never calls it).
    pub fn load_dir(dir: &std::path::Path) -> Result<AiPack, String> {
        AiPack::load(&mut |rel| std::fs::read_to_string(dir.join(rel)).ok())
    }

    /// An enum value by enum and value name ("TRES_ENEMY_UID_BX903" or "E::V"); -1 unknown.
    pub fn enum_value(&self, en: &str, name: &str) -> i64 {
        self.enums.get(en).and_then(|m| m.get(enum_value(name))).copied().unwrap_or(-1)
    }

    /// A blueprint class by asset name or object path.
    pub fn class(&self, name_or_path: &str) -> Option<&ClassDef> {
        self.classes.get(asset_name(name_or_path))
    }
}

// ---- small JSON property readers used by the node modules ---------------------------------------

pub fn pf(p: &Props, k: &str, default: f32) -> f32 {
    match p.get(k) {
        Some(Value::Number(n)) => n.as_f64().unwrap_or(default as f64) as f32,
        // AIDataProviderFloatValue {DefaultValue: x}
        Some(Value::Object(o)) => o.get("DefaultValue").and_then(|v| v.as_f64()).map(|v| v as f32).unwrap_or(default),
        _ => default,
    }
}

pub fn pi(p: &Props, k: &str, default: i32) -> i32 {
    match p.get(k) {
        Some(Value::Number(n)) => n.as_i64().unwrap_or(default as i64) as i32,
        Some(Value::Object(o)) => o.get("DefaultValue").and_then(|v| v.as_i64()).map(|v| v as i32).unwrap_or(default),
        _ => default,
    }
}

pub fn pb(p: &Props, k: &str, default: bool) -> bool {
    match p.get(k) {
        Some(Value::Bool(b)) => *b,
        Some(Value::Object(o)) => o.get("DefaultValue").and_then(|v| v.as_bool()).unwrap_or(default),
        _ => default,
    }
}

/// An enum / name / string property ("EEnum::Value" kept whole; use `enum_value`).
pub fn ps<'a>(p: &'a Props, k: &str) -> Option<&'a str> {
    p.get(k).and_then(|v| v.as_str())
}

/// The value part of "EEnum::Value" (or the string itself).
pub fn enum_value(s: &str) -> &str {
    s.rsplit("::").next().unwrap_or(s)
}

/// A `BlackboardKeySelector`'s key name.
pub fn pkey(p: &Props, k: &str) -> Option<String> {
    match p.get(k) {
        Some(Value::Object(o)) => o.get("SelectedKeyName").and_then(|v| v.as_str()).filter(|s| *s != "None").map(String::from),
        _ => None,
    }
}

/// A `GameplayTag` property's tag name.
pub fn ptag(p: &Props, k: &str) -> Option<String> {
    match p.get(k) {
        Some(Value::Object(o)) => o.get("TagName").and_then(|v| v.as_str()).filter(|s| *s != "None").map(String::from),
        _ => None,
    }
}

/// An object / class reference property as an asset name ("n_ex001_Attack4_MagFire").
pub fn passet(p: &Props, k: &str) -> Option<String> {
    p.get(k).and_then(|v| v.as_str()).map(|s| asset_name(s).trim_end_matches("_C").to_string())
}

/// A native class reference ("/Script/TresGame.TresEnvQueryContext_Querier" -> "TresEnvQueryContext_Querier").
pub fn pnative(p: &Props, k: &str) -> Option<String> {
    p.get(k).and_then(|v| v.as_str()).map(|s| s.rsplit('.').next().unwrap_or(s).trim_end_matches("_C").to_string())
}

pub fn parr<'a>(p: &'a Props, k: &str) -> &'a [Value] {
    p.get(k).and_then(|v| v.as_array()).map(|v| v.as_slice()).unwrap_or(&[])
}

pub fn pobj<'a>(p: &'a Props, k: &str) -> Option<&'a Props> {
    p.get(k).and_then(|v| v.as_object())
}
