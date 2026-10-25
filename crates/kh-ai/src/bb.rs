//! The blackboard (`UBlackboardComponent` over a `UBlackboardData` chain) [U: stock UE 4.17].
//!
//! Keys come from the asset and its parents (parent keys first, as UE numbers them). Values are
//! typed as the key types are; a change is recorded so the behaviour tree can run its observers
//! (`BTDecorator_Blackboard` and the Tres decorators that observe a key).

use crate::api::ActorRef;
use crate::pack::{AiPack, BlackboardDef};
use glam::Vec3;

pub type KeyId = u16;

/// `FAISystem::InvalidLocation` (UE: FLT_MAX in every component).
pub const INVALID_LOCATION: Vec3 = Vec3::splat(f32::MAX);

pub fn is_valid_location(v: Vec3) -> bool {
    v.x != f32::MAX && v.y != f32::MAX && v.z != f32::MAX
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyType {
    Bool,
    Int,
    Float,
    Enum,
    Name,
    String,
    Vector,
    Rotator,
    Object,
    Class,
}

/// What an Object key holds: an actor, a lock-on component of an actor (`LockOnTargetComponent`,
/// part index), or an asset (an `EnvQuery` in `ActionEQSQuery` / `FollowPlayerEQSQuery`).
#[derive(Clone, Debug, PartialEq, Default)]
pub enum BbObject {
    #[default]
    None,
    Actor(ActorRef),
    /// A scene component of an actor (lock-on target components): actor + component index.
    Component(ActorRef, u8),
    Asset(String),
}

impl BbObject {
    pub fn is_none(&self) -> bool {
        matches!(self, BbObject::None) || matches!(self, BbObject::Actor(a) if a.is_none())
    }
    /// The actor an Object value designates (a component designates its owner).
    pub fn actor(&self) -> ActorRef {
        match self {
            BbObject::Actor(a) | BbObject::Component(a, _) => *a,
            _ => ActorRef::None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum BbValue {
    Bool(bool),
    Int(i32),
    Float(f32),
    Enum(u8),
    Name(String),
    Str(String),
    Vector(Vec3),
    Rotator(Vec3),
    Object(BbObject),
    /// A class: blueprint asset name ("n_ex001_Move1") or None.
    Class(Option<String>),
}

#[derive(Clone, Debug)]
pub struct Key {
    pub name: String,
    pub ty: KeyType,
}

#[derive(Clone, Debug)]
pub struct Blackboard {
    pub asset: String,
    keys: Vec<Key>,
    values: Vec<BbValue>,
    /// Keys changed since the observers last ran (in change order, unique).
    changed: Vec<KeyId>,
}

pub fn default_value(ty: KeyType) -> BbValue {
    match ty {
        KeyType::Bool => BbValue::Bool(false),
        KeyType::Int => BbValue::Int(0),
        KeyType::Float => BbValue::Float(0.0),
        KeyType::Enum => BbValue::Enum(0),
        KeyType::Name => BbValue::Name(String::new()),
        KeyType::String => BbValue::Str(String::new()),
        // UE: vector / rotator keys start invalid (FAISystem::InvalidLocation / InvalidRotation)
        KeyType::Vector => BbValue::Vector(INVALID_LOCATION),
        KeyType::Rotator => BbValue::Rotator(INVALID_LOCATION),
        KeyType::Object => BbValue::Object(BbObject::None),
        KeyType::Class => BbValue::Class(None),
    }
}

fn key_type(s: &str) -> KeyType {
    match s {
        "Bool" => KeyType::Bool,
        "Int" => KeyType::Int,
        "Float" => KeyType::Float,
        "Enum" | "NativeEnum" => KeyType::Enum,
        "Name" => KeyType::Name,
        "String" => KeyType::String,
        "Vector" => KeyType::Vector,
        "Rotator" => KeyType::Rotator,
        "Class" => KeyType::Class,
        _ => KeyType::Object,
    }
}

impl Blackboard {
    /// The blackboard of `asset` with its parent chain (parent keys first).
    pub fn new(pack: &AiPack, asset: &str) -> Blackboard {
        let mut chain: Vec<&BlackboardDef> = Vec::new();
        let mut cur = pack.blackboards.get(asset);
        while let Some(b) = cur {
            chain.push(b);
            cur = b.parent.as_ref().and_then(|p| pack.blackboards.get(p));
            if chain.len() > 8 {
                break;
            }
        }
        let mut keys = Vec::new();
        for b in chain.iter().rev() {
            for k in &b.keys {
                if !keys.iter().any(|x: &Key| x.name == k.name) {
                    keys.push(Key { name: k.name.clone(), ty: key_type(&k.ty) });
                }
            }
        }
        let values = keys.iter().map(|k| default_value(k.ty)).collect();
        Blackboard { asset: asset.to_string(), keys, values, changed: Vec::new() }
    }

    pub fn keys(&self) -> &[Key] {
        &self.keys
    }

    pub fn key_id(&self, name: &str) -> Option<KeyId> {
        self.keys.iter().position(|k| k.name == name).map(|i| i as KeyId)
    }

    pub fn key_type(&self, id: KeyId) -> Option<KeyType> {
        self.keys.get(id as usize).map(|k| k.ty)
    }

    pub fn key_name(&self, id: KeyId) -> &str {
        self.keys.get(id as usize).map(|k| k.name.as_str()).unwrap_or("?")
    }

    pub fn get(&self, id: KeyId) -> Option<&BbValue> {
        self.values.get(id as usize)
    }

    pub fn get_by_name(&self, name: &str) -> Option<&BbValue> {
        self.key_id(name).and_then(|i| self.get(i))
    }

    /// Set a value; records a change when it differs (UE notifies observers only on change).
    pub fn set(&mut self, id: KeyId, v: BbValue) {
        if let Some(slot) = self.values.get_mut(id as usize) {
            if *slot != v {
                *slot = v;
                if !self.changed.contains(&id) {
                    self.changed.push(id);
                }
            }
        }
    }

    pub fn set_by_name(&mut self, name: &str, v: BbValue) {
        if let Some(id) = self.key_id(name) {
            self.set(id, v);
        }
    }

    /// `ClearValue`: back to the key type's default (invalid for vectors, None for objects).
    pub fn clear(&mut self, id: KeyId) {
        if let Some(ty) = self.key_type(id) {
            self.set(id, default_value(ty));
        }
    }

    pub fn take_changes(&mut self) -> Vec<KeyId> {
        std::mem::take(&mut self.changed)
    }

    pub fn has_changes(&self) -> bool {
        !self.changed.is_empty()
    }

    // ---- typed reads (UE's GetValueAs*) ----

    pub fn bool(&self, id: KeyId) -> bool {
        match self.get(id) {
            Some(BbValue::Bool(b)) => *b,
            Some(BbValue::Int(i)) => *i != 0,
            Some(BbValue::Float(f)) => *f != 0.0,
            Some(BbValue::Object(o)) => !o.is_none(),
            _ => false,
        }
    }

    pub fn float(&self, id: KeyId) -> f32 {
        match self.get(id) {
            Some(BbValue::Float(f)) => *f,
            Some(BbValue::Int(i)) => *i as f32,
            Some(BbValue::Bool(b)) => *b as i32 as f32,
            Some(BbValue::Enum(e)) => *e as f32,
            _ => 0.0,
        }
    }

    pub fn object(&self, id: KeyId) -> BbObject {
        match self.get(id) {
            Some(BbValue::Object(o)) => o.clone(),
            _ => BbObject::None,
        }
    }

    pub fn actor(&self, id: KeyId) -> ActorRef {
        self.object(id).actor()
    }

    pub fn vector(&self, id: KeyId) -> Vec3 {
        match self.get(id) {
            Some(BbValue::Vector(v)) | Some(BbValue::Rotator(v)) => *v,
            _ => INVALID_LOCATION,
        }
    }

    pub fn class(&self, id: KeyId) -> Option<String> {
        match self.get(id) {
            Some(BbValue::Class(c)) => c.clone(),
            _ => None,
        }
    }

    /// A readable value for traces.
    pub fn describe(&self, id: KeyId) -> String {
        match self.get(id) {
            Some(BbValue::Bool(b)) => b.to_string(),
            Some(BbValue::Int(i)) => i.to_string(),
            Some(BbValue::Float(f)) => format!("{f:.2}"),
            Some(BbValue::Enum(e)) => e.to_string(),
            Some(BbValue::Name(s)) | Some(BbValue::Str(s)) => s.clone(),
            Some(BbValue::Vector(v)) | Some(BbValue::Rotator(v)) => {
                if is_valid_location(*v) {
                    format!("({:.0},{:.0},{:.0})", v.x, v.y, v.z)
                } else {
                    "invalid".into()
                }
            }
            Some(BbValue::Object(o)) => format!("{o:?}"),
            Some(BbValue::Class(c)) => c.clone().unwrap_or_else(|| "None".into()),
            None => "?".into(),
        }
    }
}
