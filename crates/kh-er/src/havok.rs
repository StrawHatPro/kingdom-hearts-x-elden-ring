//! Shape casts through the game's Havok world (`hknpWorld`), after erfps2's `raycast.rs`
//! (Dasaav-dsv/erfps2 0.3.5, MIT OR Apache-2.0): the game's own cast function is called with a
//! hand-built parameter block and a hit collector whose vtable is ours.
//!
//! Every address is found by PATTERN in the running executable and must equal the value recorded
//! for 2.7.1.0 below; on any difference the casts stay off (the DLL logs and Sora is not run).
//!
//! Elden Ring space here (metres, Havok positions). Game thread only.

use std::ffi::c_void;
use std::mem::MaybeUninit;
use std::sync::OnceLock;

use eldenring::cs::{CSHavokMan, CSPhysIns, FieldInsType};
use fromsoftware_shared::FromStatic;
use glam::Vec3;
use pelite::pattern;
use pelite::pe64::Pe;

use crate::entry::{exe_base, exe_view};
use crate::erlog;

/// The collision filter word the game's camera casts with (erfps2 uses the same): map collision,
/// objects and characters answer it. Characters are rejected by owner type in `cast_sphere`.
pub const FILTER_CAMERA: u32 = 0x0200_0058;

/// WHAT SORA'S CASTS USE: layer 0x56 in place of the camera's 0x58. The low byte of the word is a LAYER
/// NUMBER (period 0x80), not a set of bits. MEASURED with the layer scan (`w.scan`, 2.7.1.0, IMPL_ER3_STATE.md):
/// layers 0x53 0x55 0x56 0x5e 0x5f 0x67 hit the very same ground bodies at the same heights as the
/// camera's 0x58, but NOT the water surface (a body the camera's layer does hit, 2 m under a shore
/// meadow and on top of the sea): with 0x56 Sora stands on the bed with the water drawn over his
/// feet, and walks on down under the sea where it gets deep. (The player's own controller layer
/// was not read: reading its Havok proxy from a frame task hung the game.)
/// (WALLS) NO LONGER the default of the KH characters' world (that is `FILTER_PLAYER`); still the enemy puppets' fallback
/// and one of the water rule's solid layers (sora.rs `WATER_SOLID_OLD`).
pub const FILTER_SORA: u32 = 0x0200_0056;

/// (WALLS 2026-10-23) THE TARNISHED'S OWN COLLISION WORD, READ IN THE GAME'S CODE (2.7.2.0, byte-identical in 2.7.1.0;
/// build/sora_tmp/party/IMPL_WALLS_STATE.md): the player's physics module (init 0x460690) fills its character proxy's
/// `collisionFilterInfo` in 0x462540: layer 2 for the local player (3 for player-type characters with event id
/// 10000000-10000039, 1 for ghosts), `| 1 << 25`. The proxy (hknpCharacterProxy, 0x18a5fe0 copies the word to +0x3c)
/// moves by calling THE SAME shape cast `cast_sphere` calls (0x1880f50) with that word, against the same filter
/// (CS::CSCollisionFilter: word bits 0-6 = layer, a 128 x 128 layer matrix built in 0x5f6b0, rebuilt offline:
/// WALLS/matrix_2720.txt). Layer 2 meets the map layers 0x35 0x38-0x3a 0x3d-0x3f 0x42-0x44 0x47-0x49 0x4b 0x52 0x54 0x69:
/// 0x35 is the CHARACTER-ONLY collision (every character proxy meets it, the camera's 0x58 and the old 0x56 do not: the
/// invisible walls); 0x39 0x3e 0x43 0x48 the characters' (hi) mesh. 0x56 met instead the camera-only mesh 0x37 0x3c 0x41
/// 0x46 and 0x3b 0x40 0x45 0x4a 0x4c, which no character proxy meets. Like 0x56, it does not meet the water surface (0x33).
/// Characters (2-4, 0xb-0x10, 0x14, ragdolls 0x1f-0x2b) answer it too; `cast_sphere` drops them by owner type.
pub const FILTER_PLAYER: u32 = 0x0200_0002;

/// (WALLS) The filter word of a cast with layer `layer` (the system-group bit of every word the game builds, `1 << 25`).
pub const fn filter_of_layer(layer: u32) -> u32 {
    0x0200_0000 | (layer & 0x7f)
}

/// One function of the executable: the pattern that finds it (pelite syntax; the wanted address
/// is the capture numbered `capture`) and the RVA it must have in each supported version.
/// (PORT) `rva_2_7_2_0`: the 2.7.2.0 RVA (build/sora_tmp/er/port272/INVENTORY.md); 0 = no recorded value.
pub(crate) struct Located {
    pub name: &'static str,
    pub pattern: &'static str,
    pub capture: usize,
    pub rva_2_7_1_0: u32,
    pub rva_2_7_2_0: u32,
}

impl Located {
    /// (PORT) The recorded RVA for a version (None: no value recorded for it).
    pub fn recorded(&self, version: Option<crate::gamever::ExeVersion>) -> Option<u32> {
        crate::gamever::pick_for(version, self.rva_2_7_1_0 as usize, self.rva_2_7_2_0 as usize).map(|v| v as u32)
    }
}

/// (PORT) THE RESOLUTION RULE, for the running executable and for the offline check of both files alike: the pattern
/// must match ONE place (captures compared), and that place must be the version's recorded RVA when one is recorded.
/// A version without a recorded value takes the unique match (the pattern is the authority; a future patch then
/// breaks only where the code itself changed). `Err` = the feature that needs it stays off.
pub(crate) fn resolve_in<'a, P: Pe<'a>>(pe: P, item: &Located, version: Option<crate::gamever::ExeVersion>) -> Result<u32, String> {
    let atoms = pattern::parse(item.pattern).map_err(|e| format!("pattern for {} does not parse: {e}", item.name))?;
    let mut save = [0u32; 4];
    let mut matches = pe.scanner().matches_code(&atoms);
    let mut found: Vec<u32> = Vec::new();
    while matches.next(&mut save) {
        if !found.contains(&save[item.capture]) {
            found.push(save[item.capture]);
        }
        if found.len() > 4 {
            break;
        }
    }
    let vname = version.map(crate::gamever::ExeVersion::name).unwrap_or("an unknown version");
    match (found.as_slice(), item.recorded(version)) {
        ([rva], Some(want)) if *rva == want => Ok(*rva),
        ([rva], Some(want)) => Err(format!("{} found at RVA {rva:#x} but {vname} has it at {want:#x}: not used", item.name)),
        ([rva], None) => Ok(*rva),
        ([], _) => Err(format!("pattern for {} not found ({vname})", item.name)),
        (many, _) => Err(format!("pattern for {} is ambiguous ({vname}): {many:x?}", item.name)),
    }
}

/// The hit collector constructor and the shape cast, as the camera's wall check calls them.
const PAT_COLLECTOR_AND_CAST: &str =
    "e8 $ { ' } 90 48 8d 45 50 48 89 44 24 20 4c 8d 8d f0 00 00 00 4c 8d 05 ? ? ? ? 48 8d 54 24 60 49 8b 4f 08 e8 $ { ' }";
/// `hknpSphereShape` constructor.
const PAT_SPHERE_SHAPE: &str = "e8 $ { ' } 48 8b d8 eb 02 33 db 0f b7 44 24 30 66 89 43 10";

pub(crate) const LOCATED: [Located; 3] = [
    Located { name: "hit collector constructor", pattern: PAT_COLLECTOR_AND_CAST, capture: 1, rva_2_7_1_0: 0x00c5_a1c0, rva_2_7_2_0: 0x00c5_a680 /* (PORT) */ },
    Located { name: "hknpWorld shape cast", pattern: PAT_COLLECTOR_AND_CAST, capture: 2, rva_2_7_1_0: 0x0187_f860, rva_2_7_2_0: 0x0188_0f50 /* (PORT) */ },
    Located { name: "hknpSphereShape constructor", pattern: PAT_SPHERE_SHAPE, capture: 1, rva_2_7_1_0: 0x0188_5b30, rva_2_7_2_0: 0x0188_7220 /* (PORT) */ },
];

#[derive(Clone, Copy)]
struct Functions {
    collector_ctor: usize,
    cast_shape: usize,
    sphere_ctor: usize,
}

static FUNCTIONS: OnceLock<Option<Functions>> = OnceLock::new();

pub(crate) fn locate(item: &Located) -> Option<usize> {
    // (PORT) the rule of `resolve_in`, for the running executable's version
    let version = crate::gamever::current();
    match resolve_in(exe_view(), item, version) {
        Ok(rva) => {
            if item.recorded(version).is_none() {
                erlog!("locate: {} taken by its unique pattern alone at RVA {rva:#x} (no value recorded for {})", item.name, crate::gamever::current_name());
            }
            Some(exe_base() + rva as usize)
        }
        Err(why) => {
            erlog!("locate: {why} (feature off)");
            None
        }
    }
}

/// Finds the functions (once). False when the casts cannot be used.
pub fn init() -> bool {
    FUNCTIONS
        .get_or_init(|| {
            let (a, b, c) = (locate(&LOCATED[0])?, locate(&LOCATED[1])?, locate(&LOCATED[2])?);
            erlog!("havok: collector ctor, shape cast and sphere ctor located by pattern at the recorded RVAs (PORT)");
            Some(Functions { collector_ctor: a, cast_shape: b, sphere_ctor: c })
        })
        .is_some()
}

#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, Default)]
struct V4([f32; 4]);

impl V4 {
    fn of(v: Vec3, w: f32) -> Self {
        Self([v.x, v.y, v.z, w])
    }
    fn xyz(&self) -> Vec3 {
        Vec3::new(self.0[0], self.0[1], self.0[2])
    }
}

#[repr(C)]
struct CastParams {
    shape_tag_filter: *mut c_void,
    collision_filter: *mut c_void,
    unk10: u16,
    filter: u32,
    user_data: *mut c_void,
    unk20: u32,
    unk24: u8,
    shape: *mut SphereShape,
    origin: V4,
    direction: V4,
    direction_rcp: V4,
    unk60: f32,
    unk64: u32,
    unk68: u32,
    unk6c: u32,
    unk70: *mut c_void,
    unk78: f32,
    unk80: V4,
    unk90: V4,
}

/// `hknpCollisionResult` as the collector receives it.
#[repr(C)]
#[derive(Clone, Copy)]
struct HknpHit {
    pos: V4,
    normal: V4,
    fraction: f32,
    unk24: [u8; 4],
    unk28: u32,
    unk2c: u16,
    unk30: u32,
    filter: u32,
    unk38: u32,
    unk3c: [u8; 4],
    unk40: [u8; 8],
    body_id: u32,
    unk4c: u32,
    unk50: u32,
    unk54: u32,
    unk58: u32,
    body: *mut *mut CSPhysIns,
    unk68: u32,
    unk6c: u32,
}

#[repr(C)]
struct Collector {
    vtable: *const CollectorVtable,
    unk08: u8,
    is_hit: u32,
    segment: V4,
    unk20: u8,
    hit: HknpHit,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct CollectorVtable {
    hk_reflection: usize,
    drop: usize,
    drop_in_place: usize,
    collision: usize,
    add_hit: unsafe extern "C" fn(*mut Collector, *const HknpHit),
    unk28: usize,
}

/// Our vtable: the game's functions with `add_hit` replaced, followed by what our `add_hit` needs.
#[repr(C)]
struct OurVtable<'a> {
    base: CollectorVtable,
    original_add_hit: unsafe extern "C" fn(*mut Collector, *const HknpHit),
    sink: &'a mut dyn FnMut(&HknpHit) -> bool,
}

#[repr(C)]
struct SphereShape {
    vtable: usize,
    unk08: [u8; 0x8],
    size_bytes: u16,
    unk12: [u8; 0xe],
    radius: f32,
    unk24: [u8; 0x16],
    vertex_count: u16,
    vertices: [V4; 1],
}

const _: () = {
    assert!(std::mem::size_of::<CastParams>() == 0xa0);
    assert!(std::mem::size_of::<HknpHit>() == 0x70);
    assert!(std::mem::size_of::<Collector>() == 0xa0);
    assert!(std::mem::size_of::<SphereShape>() == 0x50);
    assert!(std::mem::offset_of!(HknpHit, body_id) == 0x48);
    assert!(std::mem::offset_of!(HknpHit, body) == 0x60);
    assert!(std::mem::offset_of!(CastParams, origin) == 0x30);
    assert!(std::mem::offset_of!(Collector, hit) == 0x30);
};

unsafe extern "C" fn add_hit(collector: *mut Collector, data: *const HknpHit) {
    // SAFETY: `collector` is one of ours: its vtable pointer is an `OurVtable`.
    let table = unsafe { &mut *((*collector).vtable as *mut OurVtable) };
    let accept = (table.sink)(unsafe { &*data });
    if accept {
        // SAFETY: the game's own collector function, on a collector the game constructed.
        unsafe { (table.original_add_hit)(collector, data) };
    }
}

/// One contact a cast reported.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CastHit {
    /// Fraction of the cast's displacement at which the shape touches.
    pub fraction: f32,
    /// The contact position as Havok reports it.
    pub position: Vec3,
    /// The contact normal as Havok reports it.
    pub normal: Vec3,
    /// The body touched (index part of `hknpBodyId`).
    pub body: u32,
    /// The collision filter word of what was hit.
    pub filter: u32,
    /// The kind of thing owning the body (`FieldInsType` as a number, 255 unknown).
    pub owner: u8,
    /// Words of the hit record whose meaning is not known (offsets 0x28, 0x2c, 0x30, 0x38): a
    /// shape tag or material is expected among them (for telling the water surface from ground).
    pub tags: [u32; 4],
    /// (IMPL_ER18) The object owning the body (`FieldInsBase`; 0 unknown) and the body's `CSPhysIns`: for the log only.
    pub owner_ptr: usize,
    pub phys_ptr: usize,
}

fn owner_type(hit: &HknpHit) -> u8 {
    // SAFETY: the body's user pointer, as erfps2 reads it; every level is null-checked.
    unsafe {
        let Some(phys) = hit.body.as_ref() else { return 255 };
        let Some(phys) = phys.as_ref() else { return 255 };
        match phys.owner.as_ref().handle.selector.field_ins_type() {
            Some(t) => t as u8,
            None => 255,
        }
    }
}

/// Sweeps a sphere of `radius` from `origin` along `delta` through the physics world and returns
/// EVERY contact the world reported that `keep` accepts (unordered; a body can appear more than
/// once). Characters never count. `prune`: let the game's closest-hit collector shorten the cast
/// as contacts are accepted (faster; farther contacts are then not reported).
pub fn cast_sphere(origin: Vec3, delta: Vec3, radius: f32, filter: u32, prune: bool, out: &mut Vec<CastHit>) {
    cast_sphere_of(origin, delta, radius, filter, prune, None, out)
}

/// (IMPL_ER32) The same cast keeping only the contacts on bodies of ONE owner type (`FieldInsType` as a number; 6 = a map
/// object): what else the sphere meets (the ground's triangles) does not use up the list. `None`: everything.
pub fn cast_sphere_of(origin: Vec3, delta: Vec3, radius: f32, filter: u32, prune: bool, only: Option<u8>, out: &mut Vec<CastHit>) {
    out.clear();
    let Some(Some(f)) = FUNCTIONS.get().copied() else { return };
    if delta.length_squared() < 1e-12 || !origin.is_finite() || !delta.is_finite() {
        return;
    }
    // SAFETY: game thread.
    let Ok(havok_man) = (unsafe { CSHavokMan::instance_mut() }) else { return };
    let world = havok_man.phys_world.hknp_world.as_ptr();

    // SAFETY: the game's constructors on storage of the right size (checked above); the cast is
    // the call the camera makes, with the same parameter block.
    unsafe {
        let mut shape = MaybeUninit::<SphereShape>::zeroed();
        let shape_ptr = shape.as_mut_ptr();
        (*shape_ptr).vertex_count = 1;
        let ctor: unsafe extern "C" fn(*mut SphereShape, *const V4, f32) = std::mem::transmute(f.sphere_ctor);
        ctor(shape_ptr, &V4::default(), radius);
        (*shape_ptr).size_bytes = 0x50;

        let collector_ctor: unsafe extern "C" fn(*mut Collector) = std::mem::transmute(f.collector_ctor);
        let mut ours = MaybeUninit::<Collector>::zeroed();
        collector_ctor(ours.as_mut_ptr());
        let mut second = MaybeUninit::<Collector>::zeroed();
        collector_ctor(second.as_mut_ptr());

        let mut sink = |hit: &HknpHit| -> bool {
            let owner = owner_type(hit);
            if owner == FieldInsType::Chr as u8 || only.is_some_and(|o| o != owner) {
                return false;
            }
            if out.len() < 256 {
                out.push(CastHit {
                    fraction: hit.fraction,
                    position: hit.pos.xyz(),
                    normal: hit.normal.xyz(),
                    body: hit.body_id & 0x00ff_ffff,
                    filter: hit.filter,
                    owner,
                    tags: [hit.unk28, hit.unk2c as u32, hit.unk30, hit.unk38],
                    owner_ptr: hit.body.as_ref().and_then(|p| p.as_ref()).map_or(0, |p| p.owner.as_ptr() as usize),
                    phys_ptr: hit.body.as_ref().map_or(0, |p| *p as usize),
                });
            }
            prune
        };
        let game_table = &*(*ours.as_ptr()).vtable;
        let mut table = OurVtable { base: CollectorVtable { add_hit, ..*game_table }, original_add_hit: game_table.add_hit, sink: &mut sink };
        (*ours.as_mut_ptr()).vtable = &mut table as *mut OurVtable as *const CollectorVtable;

        let rcp = |d: f32| if d == 0.0 { f32::MAX } else { d.recip() };
        let mut params = CastParams {
            shape_tag_filter: (*world).shape_tag_filter,
            collision_filter: havok_man.collision_filter,
            unk10: u16::MAX,
            filter,
            user_data: std::ptr::null_mut(),
            unk20: 2,
            unk24: 0xfb,
            shape: shape_ptr,
            origin: V4::of(origin, 1.0),
            direction: V4::of(delta, 1.0),
            direction_rcp: V4::of(Vec3::new(rcp(delta.x), rcp(delta.y), rcp(delta.z)), 1.0),
            unk60: 0.001,
            unk64: 0,
            unk68: 0,
            unk6c: 0x100,
            unk70: std::ptr::null_mut(),
            unk78: f32::MAX,
            unk80: V4::default(),
            unk90: V4::default(),
        };
        const IDENTITY: [V4; 4] = [V4([1.0, 0.0, 0.0, 0.0]), V4([0.0, 1.0, 0.0, 0.0]), V4([0.0, 0.0, 1.0, 0.0]), V4([0.0, 0.0, 0.0, 1.0])];
        let cast: unsafe extern "C" fn(*mut c_void, *mut CastParams, *const [V4; 4], *mut Collector, *mut Collector) = std::mem::transmute(f.cast_shape);
        cast(world as *mut c_void, &mut params, &IDENTITY, ours.as_mut_ptr(), second.as_mut_ptr());
    }
}

/// (WALLS) The layer facts the KH characters' collision rests on, as rows of the game's own layer matrix (rebuilt offline from
/// CS::CSCollisionFilter's init, 2.7.2.0 0x5f6b0 = 2.7.1.0 0x5f1f0, identical; build/sora_tmp/party/WALLS/matrix_2720.txt).
#[cfg(test)]
mod walls_tests {
    use super::*;

    /// The rows used here, as the rebuild wrote them (layer: the layers it meets).
    const ROWS: [(u32, &str); 6] = [
        (0x02, "0 2-4 b-c e-10 14 1f-2b 35 38-3a 3d-3f 42-44 47-49 4b 52 54 69"),
        (0x15, "0 35 38-3a 3d-3f 42-44 47-49 4b 52 54 69"),
        (0x35, "0 2-a 15-19 5d 67 69 6b 71-72"),
        (0x56, "0 1d-2d 37-38 3a-3d 3f-42 44-47 49-4c 52 54 69"),
        (0x58, "0 1d-20 23-24 27-28 33 37-38 3a-3d 3f-42 44-47 49-4a 4d 52-54"),
        (0x5c, "0 38-3b 3d-40 42-45 47-4a 52 54"),
    ];

    fn row(layer: u32) -> Vec<u32> {
        let text = ROWS.iter().find(|r| r.0 == layer).unwrap().1;
        text.split(' ')
            .flat_map(|part| {
                let mut ends = part.split('-').map(|n| u32::from_str_radix(n, 16).unwrap());
                let a = ends.next().unwrap();
                a..=ends.next().unwrap_or(a)
            })
            .collect()
    }

    #[test]
    fn the_words_are_layer_numbers_with_the_group_bit() {
        assert_eq!(filter_of_layer(2), FILTER_PLAYER);
        assert_eq!(filter_of_layer(0x56), FILTER_SORA);
        assert_eq!(filter_of_layer(0x58), FILTER_CAMERA);
        assert_eq!(filter_of_layer(0x82), FILTER_PLAYER, "period 0x80");
    }

    #[test]
    fn the_tarnished_meets_the_character_only_walls_the_old_layer_and_the_camera_did_not() {
        let (player, old, camera) = (row(2), row(0x56), row(0x58));
        assert!(player.contains(&0x35) && !old.contains(&0x35) && !camera.contains(&0x35));
        // the matrix is symmetric: the wall layer meets every character proxy (player 2, enemies 3-0xa) and no camera layer
        let wall = row(0x35);
        assert!((2..=0xa).all(|l| wall.contains(&l)) && !wall.contains(&0x56) && !wall.contains(&0x58));
        // the player's second proxy word (0x15) meets the same map layers as his first
        let map = |r: &Vec<u32>| r.iter().copied().filter(|l| *l >= 0x30).collect::<Vec<_>>();
        assert_eq!(map(&player), map(&row(0x15)));
        // neither the old layer nor the Tarnished's meets the water surface (0x33); the camera does
        assert!(!player.contains(&0x33) && !old.contains(&0x33) && camera.contains(&0x33));
        // the old floor layer took floors of 0x3b 0x40 0x45 0x4a, which the Tarnished does not stand on
        let floor = row(0x5c);
        for l in [0x3b, 0x40, 0x45, 0x4a] {
            assert!(floor.contains(&l) && old.contains(&l) && !player.contains(&l));
        }
    }
}
