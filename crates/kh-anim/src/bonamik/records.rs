//! The records of the Bonamik core as the JS evaluator (`tools/lib/bonamik/bnm_eval.js`) builds
//! them. Field names are the byte offsets of the exe's structs in lower-case hex (`f25c` = the
//! JS's `f25C` = particle + 0x25C). References between records are indices: a particle
//! reference is an index into `Scene::f0`, a link one into `Scene::f40`, a cone one into
//! `Scene::f30`, a collision object one into `Scene::objs`, a pin record one into `Scene::f758`,
//! a KDI record one into `Scene::f768`.

use super::m::{Q4, V3, Xf};

/// Particle (0x330 bytes in the exe).
#[derive(Clone, Debug)]
pub struct Particle {
    /// position
    pub f0: V3,
    /// previous position
    pub fc: V3,
    pub f20: Q4,
    pub f30: Q4,
    /// velocity
    pub f40: V3,
    pub f4c: V3,
    pub f58: V3,
    pub f64: V3,
    pub f70: V3,
    /// local force
    pub f7c: V3,
    pub f88: V3,
    pub f94: V3,
    pub fa0: V3,
    pub fac: V3,
    pub fb8: i32,
    /// is the child of a ParentChild link
    pub fbc: u8,
    /// has an anchor constraint
    pub fbd: u8,
    pub fc0: Q4,
    pub fd0: V3,
    pub fe0: V3,
    /// previous animated bone transform
    pub ff0: Q4,
    pub f100: V3,
    pub f110: V3,
    /// current animated bone transform
    pub f120: Q4,
    pub f130: V3,
    pub f140: V3,
    pub f150: V3,
    /// target quaternion
    pub f160: Q4,
    /// local translation / rotation on the bone
    pub f170: V3,
    pub f180: Q4,
    /// parent / child particle
    pub f190: Option<usize>,
    pub f198: Option<usize>,
    /// m_ParentId / m_ChildId of the asset
    pub f1a0: i32,
    pub f1a4: i32,
    /// number of ParentChild links, and their indices
    pub f1a8: usize,
    pub f1b0: [usize; 4],
    pub f1d0: V3,
    pub f1dc: V3,
    /// long-range attachment: anchor particle, rest length, strength, damping
    pub f220: Option<usize>,
    pub f228: f32,
    pub f22c: f32,
    pub f230: f32,
    /// pin record / KDI record
    pub f238: Option<usize>,
    pub f240: Option<usize>,
    /// bone index, particle index
    pub f248: usize,
    pub f24c: usize,
    pub f250: f32,
    pub f254: f32,
    pub f258: f32,
    /// radius
    pub f25c: f32,
    pub f260: f32,
    pub f264: f32,
    pub f268: f32,
    pub f26c: f32,
    pub f270: f32,
    pub f274: u8,
    pub f275: u8,
    /// kinematic, collider, multiple-collision object, touched by a collision
    pub f276: u8,
    pub f277: u8,
    pub f278: u8,
    pub f279: u8,
    pub f27c: i32,
    /// m_CollisionBatchNo (kept as the JS number: the asset stores 0xFFFFFFFF as 4294967295)
    pub f280: i64,
    pub f284: f32,
    pub f288: f32,
    pub f28c: f32,
    pub f290: f32,
    pub f294: f32,
    pub f298: f32,
    /// collision object / cone record owned by the particle
    pub f2a0: Option<usize>,
    pub f2a8: Option<usize>,
    /// group
    pub f2b8: usize,
    pub f2bc: [i32; 8],
    /// joint-twist neighbours, their signs, their count
    pub f2e0: [usize; 4],
    pub f300: [i32; 4],
    pub f310: usize,
    pub f314: f32,
    pub f318: f32,
    pub f31c: u8,
    pub f31d: u8,
    pub f320: f32,
    pub f324: f32,
    pub f328: f32,
    pub name: String,
}

/// Link record (stride 0x38).
#[derive(Clone, Copy, Debug)]
pub struct Link {
    pub f0: usize,
    pub f8: usize,
    /// m_LinkBatchNo (as the JS number)
    pub f10: i64,
    pub f14: f32,
    pub f18: f32,
    /// rest length
    pub f20: f32,
    /// is ParentChild
    pub f24: u8,
    /// hard length
    pub f26: u8,
    /// cone record
    pub f28: Option<usize>,
    /// collision object
    pub f30: Option<usize>,
}

/// Cone record (stride 0xA0).
#[derive(Clone, Copy, Debug)]
pub struct Cone {
    pub f0: usize,
    pub f8: usize,
    pub f10: i64,
    pub f14: f32,
    pub f18: f32,
    pub f20: u8,
    pub f24: usize,
    pub f28: f32,
    pub f2c: f32,
    pub f30: f32,
    pub f34: f32,
    pub f38: f32,
    pub f3c: f32,
    pub f40: f32,
    pub f44: f32,
    pub f50: Q4,
    pub f60: Q4,
    pub f70: Q4,
    pub f80: Q4,
    pub f90: V3,
}

/// Anchor constraint (stride 0x40).
#[derive(Clone, Copy, Debug)]
pub struct Anchor {
    pub f0: usize,
    pub f4: i32,
    pub f8: f32,
    pub fc: V3,
    pub f18: V3,
    pub f24: f32,
    pub f28: V3,
    pub f34: V3,
}

/// Collision object (0x88 bytes).
#[derive(Clone, Copy, Debug)]
pub struct CollObj {
    pub f0: usize,
    pub f8: Option<usize>,
    pub f10: Option<usize>,
    pub f18: i32,
    /// batch number (as the JS number)
    pub f1c: i64,
    pub f20: V3,
    pub f2c: V3,
    pub f38: usize,
    pub f3c: [i32; 8],
    pub f5c: f32,
    pub f60: f32,
    pub f64: f32,
    pub f68: V3,
    pub f74: V3,
    pub f80: f32,
}

/// Pin record (stride 0x28).
#[derive(Clone, Copy, Debug)]
pub struct Pin {
    pub f0: usize,
    pub f8: usize,
    pub f10: V3,
    pub f1c: f32,
    pub f20: f32,
}

/// Driver entry of a KDI record (stride 0x40).
#[derive(Clone, Copy, Debug)]
pub struct KdiEntry {
    pub f0: usize,
    pub f8: V3,
    pub f20: Q4,
    pub f30: f32,
    pub f34: f32,
}

/// KDI record (stride 0x18): the driven particle and its drivers (`f10` = `f8.len()`).
#[derive(Clone, Debug)]
pub struct Kdi {
    pub f0: usize,
    pub f8: Vec<KdiEntry>,
}

/// The scene (`core + 0x3778`).
#[derive(Clone, Debug, Default)]
pub struct Scene {
    /// particles (count `f8` = `f0.len()`)
    pub f0: Vec<Particle>,
    /// anchors (count `f18`)
    pub f10: Vec<Anchor>,
    /// cones (count `f38`)
    pub f30: Vec<Cone>,
    /// links (count `f48`)
    pub f40: Vec<Link>,
    /// every collision object ever built (the JS's object identities)
    pub objs: Vec<CollObj>,
    /// dynamic-side objects (count `f58`), sorted by batch at the end of setup
    pub f50: Vec<usize>,
    /// self-collision pairs (count `f68`)
    pub f60: Vec<usize>,
    /// collider lists: 0..47 per group, 48.. per collision tag
    pub f70: Vec<Vec<usize>>,
    /// chain roots (count `f750`)
    pub f748: Vec<usize>,
    /// pin records (count `f760`)
    pub f758: Vec<Pin>,
    /// KDI records (count `f770`)
    pub f768: Vec<Kdi>,
    pub f7ac: u8,
}

/// `SQEX_BonamikGlobalConfig` CDO (constructor 0x143AE8A30).
#[derive(Clone, Copy, Debug)]
pub struct Cfg {
    /// bEnableUpdate
    pub f53: u8,
    /// bEnableSimulation
    pub f55: u8,
    pub f56: u8,
    pub f58: f32,
    /// ExpectedFrameRate
    pub f5c: f32,
    /// MayaAnimationFrameRate
    pub f60: f32,
    /// MayaSimulationFrameRate
    pub f64: f32,
}

impl Default for Cfg {
    fn default() -> Cfg {
        Cfg { f53: 1, f55: 1, f56: 1, f58: (1.0_f64 / 30.0) as f32, f5c: 30.0, f60: 30.0, f64: 20.0 }
    }
}

/// The owner interface of the core (`core + 0x3848`), as the JS's `owner` object: the editable
/// component-space bone buffer and what the host supplies.
#[derive(Clone, Debug)]
pub struct Owner {
    /// `vf60()`: the editable component-space transforms.
    pub bones: Vec<Xf>,
    /// `vf50()`: the component-space reference pose of the mesh.
    pub ref_bones: Vec<Xf>,
    /// `vf78(i)`: parent bone index, -1 for the root.
    pub parents: Vec<i32>,
    pub names: Vec<String>,
    /// `vf48()`: ComponentToWorld.
    pub world: Xf,
    /// `vf20()`: gravity, world space.
    pub gravity: V3,
    /// `vf38()`: world wind.
    pub wind: V3,
    /// `vf10()`: ground query result (point, normal), world space.
    pub ground: Option<(V3, V3)>,
    /// `vf90()`: time dilation.
    pub time_dilation: f32,
}

/// The core (0x3850 bytes).
#[derive(Clone, Debug)]
pub struct Core {
    pub f0: Q4,
    pub f10: V3,
    pub f20: V3,
    pub f30: Q4,
    pub f40: V3,
    pub f50: V3,
    pub f60: i32,
    pub f70: Q4,
    pub f80: V3,
    pub f90: V3,
    pub fa0: Q4,
    pub fb0: V3,
    pub fc0: V3,
    pub fd0: Q4,
    pub fe0: V3,
    pub ff0: V3,
    pub f100: Q4,
    pub f110: V3,
    pub f120: V3,
    pub f130: Q4,
    pub f140: V3,
    pub f150: V3,
    pub f160: Q4,
    pub f170: V3,
    pub f180: V3,
    pub f190: Q4,
    pub f1a0: V3,
    pub f1b0: V3,
    /// per group: reference up vector
    pub f1c0: Vec<V3>,
    /// per bone: driver particle index + 1 (0 none, 9999 static)
    pub f400: Vec<usize>,
    /// number of groups
    pub f410: usize,
    pub f414: Vec<i32>,
    pub f4d8: Vec<String>,
    /// asset version
    pub f658: u32,
    pub f65c: u8,
    pub f65d: u8,
    pub f660: i32,
    pub f664: i32,
    pub f668: f32,
    pub f66c: f32,
    pub f670: f32,
    /// sub-steps, outer iterations, link iterations, collision iterations
    pub f674: usize,
    pub f678: usize,
    pub f67c: usize,
    pub f680: usize,
    pub f684: Vec<i32>,
    pub f744: Vec<i32>,
    /// per group: state (2 simulated, 0 off)
    pub f804: Vec<i32>,
    pub f8c4: Vec<u8>,
    pub f8f4: Vec<u8>,
    pub f924: Vec<u8>,
    pub f954: Vec<f32>,
    pub fa14: Vec<f32>,
    pub fad4: Vec<i32>,
    pub fb94: Vec<f32>,
    pub fc54: Vec<f32>,
    pub fd14: Vec<f32>,
    pub fdd4: Vec<f32>,
    pub fe94: Vec<f32>,
    pub f33e8: u8,
    pub f33e9: u8,
    pub f33ea: u8,
    pub f33ec: i32,
    pub f33f0: i32,
    /// reset pending, reset hard, reinitialise
    pub f33f4: u8,
    pub f33f5: u8,
    pub f33f6: u8,
    pub f33f8: Vec<f32>,
    pub f34b8: Vec<f32>,
    pub f3578: Vec<f32>,
    pub f3638: Vec<f32>,
    pub f36f8: f32,
    pub f36fc: f32,
    pub f3700: f32,
    pub f3704: u8,
    pub f3708: f32,
    pub f370c: f32,
    pub f3710: f32,
    pub f3714: f32,
    pub f3718: u8,
    pub f371c: f32,
    /// character wind: direction space, direction, (reference core: always this core here),
    /// base strength, wave amplitude, 1 / wave period, active
    pub f3720: i32,
    pub f3724: V3,
    pub f3738: f32,
    pub f373c: f32,
    pub f3740: f32,
    pub f3744: u8,
    /// colliders of other cores (none)
    pub f3750: usize,
    /// follower-bone lists (none)
    pub f3770: usize,
    pub f3778: Scene,
    /// ground-attach particles
    pub f37e8: usize,
    /// driven-bone bit set
    pub f3800: Vec<u32>,
    pub f3808: usize,
    /// per-bone weight
    pub f3810: Vec<f32>,
    pub f3818: usize,
    pub f3820: usize,
    pub f3824: usize,
    pub f3828: i32,
    pub f382c: f32,
    pub f3830: f32,
    pub f3834: f32,
    pub f3838: f32,
    pub f383c: f32,
    /// blend deferred to the component
    pub f3840: u8,
    pub f3848: Owner,
}

/// What `coreUpdate` counts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    pub steps: u32,
    pub resets: u32,
}
