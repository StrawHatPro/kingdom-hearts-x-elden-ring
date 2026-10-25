//! Small accessors over the SDK: the local player, the camera, the debug drawer. Game thread only.

use eldenring::cs::{CSCamera, CSEzDraw, CSTaskGroupIndex, PlayerIns, RendMan, WorldChrMan};
use eldenring::position::HavokPosition;
use eldenring::rotation::Quaternion;
use fromsoftware_shared::{F32Vector4, FromStatic};
use glam::{Quat, Vec3};

/// The frame phases the loader forwards, as the numbers it passes.
pub mod phase {
    use super::CSTaskGroupIndex as G;
    pub const FRAME_BEGIN: u32 = G::FrameBegin as u32;
    pub const END_SHIFT_WORLD: u32 = G::EndShiftWorldPosition as u32;
    pub const PRE_PHYSICS: u32 = G::ChrIns_PrePhysics as u32;
    pub const PRE_PHYSICS_END: u32 = G::ChrIns_PrePhysics_End as u32;
    pub const HAVOK_POST: u32 = G::HavokWorldUpdate_Post as u32;
    pub const CAMERA: u32 = G::CameraStep as u32;
    pub const POST_PHYSICS: u32 = G::ChrIns_PostPhysics as u32;
    pub const BEGIN_SHIFT_WORLD: u32 = G::BeginShiftWorldPosition as u32;
}

pub fn hp(v: Vec3) -> HavokPosition {
    HavokPosition(v.x, v.y, v.z, 1.0)
}

pub fn v3(p: &HavokPosition) -> Vec3 {
    Vec3::new(p.0, p.1, p.2)
}

pub fn quat(q: &Quaternion) -> Quat {
    Quat::from_xyzw(q.0, q.1, q.2, q.3)
}

pub fn quaternion(q: Quat) -> Quaternion {
    Quaternion(q.x, q.y, q.z, q.w)
}

/// A pointer word that can be a live heap object: not null, canonical user space, aligned.
pub(crate) fn plausible(word: usize) -> bool {
    (0x10000..0x7FFF_FFFF_0000).contains(&word) && word % 8 == 0
}

/// Reads the pointer word stored in a pointer-sized field without forming a reference to what it
/// points at.
#[macro_export]
macro_rules! word {
    ($place:expr) => {
        // SAFETY: the field is pointer-sized (`OwnedPtr` / `NonNull` / `Option<NonNull>`), inside a live object.
        {
            #[allow(unused_unsafe)]
            let word = unsafe { std::ptr::read_unaligned(std::ptr::addr_of!($place) as *const usize) };
            word
        }
    };
}

/// The player object is complete: every pointer we follow is set. While the game builds or tears
/// down the player (loading, death and respawn) `main_player` is already / still set but parts of
/// it are not; touching them then is an access violation that no `catch_unwind` can stop
/// (it happened: state note, "crash 1").
fn player_complete(player: &PlayerIns) -> bool {
    let chr = &player.chr_ins;
    if !(plausible(word!(chr.modules)) && plausible(word!(chr.chr_ctrl)) && plausible(word!(chr.chr_model_ins)) && plausible(word!(chr.special_effect))) {
        return false;
    }
    let m = &*chr.modules;
    let modules_ok = [word!(m.data), word!(m.physics), word!(m.fall), word!(m.time_act), word!(m.behavior), word!(m.action_request)]
        .into_iter()
        .all(plausible);
    if !modules_ok {
        return false;
    }
    let body = &chr.chr_model_ins.model_ins;
    if !(plausible(word!(body.model_disp_entity)) && plausible(word!(chr.modules.physics.owner))) {
        return false;
    }
    // The equipment models: absent is fine, a half-built table is not.
    let asm = word!(player.chr_asm_model_ins);
    if asm != 0 {
        if !plausible(asm) {
            return false;
        }
        if let Some(asm) = player.chr_asm_model_ins.as_ref() {
            for part in asm.parts_model_ins.iter() {
                let w = word!(*part);
                if w != 0 && !plausible(w) {
                    return false;
                }
                if let Some(part) = part {
                    if !plausible(word!(part.model_disp_entity)) {
                        return false;
                    }
                }
            }
        }
    }
    true
}

/// How many frames in a row the same complete player object has been seen.
static PLAYER_SEEN: std::sync::Mutex<(usize, u32)> = std::sync::Mutex::new((0, 0));

/// Call once per frame (first phase): tracks how long the current player object has existed.
pub fn track_player() {
    // SAFETY: game thread; read only.
    let now = unsafe { WorldChrMan::instance() }
        .ok()
        .and_then(|w| w.main_player.as_deref())
        .filter(|p| player_complete(p))
        .map(|p| p as *const PlayerIns as usize)
        .unwrap_or(0);
    let mut seen = PLAYER_SEEN.lock().unwrap();
    if now == 0 || seen.0 != now {
        *seen = (now, 0);
    } else {
        seen.1 = seen.1.saturating_add(1);
    }
}

/// Frames the current player object has been complete for (0 = none).
pub fn player_age() -> u32 {
    PLAYER_SEEN.lock().unwrap().1
}

/// The local player, when the world holds one that is complete and has been for half a second.
pub fn player_mut() -> Option<&'static mut PlayerIns> {
    if player_age() < 30 {
        return None;
    }
    // SAFETY: called on the game thread from a task; nothing else of ours holds a reference.
    let player = unsafe { WorldChrMan::instance_mut() }.ok()?.main_player.as_deref_mut()?;
    player_complete(player).then_some(player)
}

/// Alive and not in the middle of dying.
pub fn player_alive(player: &PlayerIns) -> bool {
    player.chr_ins.modules.data.hp > 0 && !player.chr_ins.chr_flags1c5.death_flag()
}

/// The camera the frame is drawn with: rows right, up, forward, position (Elden Ring space).
#[derive(Clone, Copy, Debug)]
pub struct Camera {
    pub right: Vec3,
    pub up: Vec3,
    pub forward: Vec3,
    pub position: Vec3,
    pub fov: f32,
    pub aspect: f32,
    pub near: f32,
    pub far: f32,
}

pub fn camera() -> Option<Camera> {
    // SAFETY: game thread; read only.
    let camera = unsafe { CSCamera::instance() }.ok()?;
    let cam = &camera.pers_cam_1;
    let m = &cam.matrix;
    Some(Camera {
        right: Vec3::new(m.0 .0, m.0 .1, m.0 .2),
        up: Vec3::new(m.1 .0, m.1 .1, m.1 .2),
        forward: Vec3::new(m.2 .0, m.2 .1, m.2 .2),
        position: Vec3::new(m.3 .0, m.3 .1, m.3 .2),
        fov: cam.fov,
        aspect: cam.aspect_ratio,
        near: cam.near_plane,
        far: cam.far_plane,
    })
}

/// The game's own debug drawer (`CSEzDraw`): world-space lines and wire shapes, drawn by the game
/// in its debug pass with its own depth test. Positions in Elden Ring space.
pub struct Draw {
    ez: &'static mut CSEzDraw,
}

pub const WHITE: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
pub const RED: [f32; 4] = [1.0, 0.1, 0.1, 1.0];
pub const GREEN: [f32; 4] = [0.1, 1.0, 0.1, 1.0];
pub const BLUE: [f32; 4] = [0.2, 0.4, 1.0, 1.0];
pub const YELLOW: [f32; 4] = [1.0, 1.0, 0.0, 1.0];
pub const CYAN: [f32; 4] = [0.0, 1.0, 1.0, 1.0];
pub const MAGENTA: [f32; 4] = [1.0, 0.0, 1.0, 1.0];
pub const ORANGE: [f32; 4] = [1.0, 0.55, 0.0, 1.0];

impl Draw {
    pub fn get() -> Option<Self> {
        // SAFETY: game thread.
        let rend = unsafe { RendMan::instance_mut() }.ok()?;
        Some(Self { ez: rend.debug_ez_draw.as_mut() })
    }

    pub fn colour(&mut self, c: [f32; 4]) {
        self.ez.set_color(&F32Vector4(c[0], c[1], c[2], c[3]));
    }

    pub fn line(&mut self, a: Vec3, b: Vec3) {
        self.ez.draw_line(&hp(a), &hp(b));
    }

    /// An upright wire capsule from its centre, radius and half height (centre to the end of the
    /// round caps), drawn as lines (the game's own capsule primitive draws something else: see
    /// the state note).
    pub fn capsule(&mut self, centre: Vec3, radius: f32, half_height: f32) {
        let straight = (half_height - radius).max(0.0);
        let (top, bottom) = (centre + Vec3::Y * straight, centre - Vec3::Y * straight);
        const N: usize = 12;
        let ring = |i: usize| {
            let a = i as f32 / N as f32 * std::f32::consts::TAU;
            Vec3::new(a.cos(), 0.0, a.sin()) * radius
        };
        for i in 0..N {
            let (p, q) = (ring(i), ring(i + 1));
            self.line(top + p, top + q);
            self.line(bottom + p, bottom + q);
            if i % 3 == 0 {
                self.line(top + p, bottom + p);
            }
        }
        // The caps: two half circles each.
        for axis in [Vec3::X, Vec3::Z] {
            const M: usize = 8;
            for i in 0..M {
                let (a, b) = (i as f32 / M as f32 * std::f32::consts::PI, (i + 1) as f32 / M as f32 * std::f32::consts::PI);
                let arc = |t: f32| axis * t.cos() * radius + Vec3::Y * t.sin() * radius;
                self.line(top + arc(a), top + arc(b));
                self.line(bottom - arc(a), bottom - arc(b));
            }
        }
    }

    /// A small three-axis cross.
    pub fn cross(&mut self, at: Vec3, size: f32) {
        for axis in [Vec3::X, Vec3::Y, Vec3::Z] {
            self.line(at - axis * size, at + axis * size);
        }
    }
}

/// A character object is complete enough to read: its module container, the data and physics
/// modules and its controller are set (characters being built or torn down are skipped).
pub fn chr_complete(chr: &eldenring::cs::ChrIns) -> bool {
    if !(plausible(word!(chr.modules)) && plausible(word!(chr.chr_ctrl)) && plausible(word!(chr.special_effect))) {
        return false;
    }
    let m = &*chr.modules;
    [word!(m.data), word!(m.physics), word!(m.time_act)].into_iter().all(plausible) && plausible(word!(chr.chr_ctrl.modifier))
}

/// The 8 bytes of a handle (selector + block id), for use as a key.
pub fn handle_bits(handle: &eldenring::cs::FieldInsHandle) -> u64 {
    // SAFETY: FieldInsHandle is repr(C, align(8)) of two 4-byte fields.
    unsafe { std::mem::transmute_copy::<eldenring::cs::FieldInsHandle, u64>(handle) }
}

/// (IMPL_ER10) The state word of the game's real-time cutscene manager (`CSRemo -> remo_man + 0x40`; the
/// SDK's `CSRemoMan::state`), `None` while there is no manager.
pub fn remo_state() -> Option<i32> {
    use eldenring::cs::CSRemo;
    use fromsoftware_shared::FromStatic;
    // SAFETY: game thread; the singleton lives as long as the game; the pointer is checked before it is followed.
    let remo = unsafe { CSRemo::instance() }.ok()?;
    let man = word!(remo.remo_man);
    if !plausible(man) {
        return None;
    }
    // SAFETY: an i32 inside the live manager object.
    Some(unsafe { std::ptr::read_unaligned((man + 0x40) as *const i32) })
}

/// The move is the physics origin shifting under the world (multiples of 8 m on every axis: MEASURED
/// (32, -32, 176), (24, 0, -32), (8, 8, -32), (56, 0, -16)), not the game putting its player elsewhere.
pub fn origin_shift(moved: Vec3) -> bool {
    let whole = |v: f32| ((v / 8.0).round() * 8.0 - v).abs() < 0.05;
    whole(moved.x) && whole(moved.y) && whole(moved.z)
}

/// (testing aid, `dbg.remo`) The cutscene manager's first bytes and the game's visible interface elements, for
/// finding what tells a cutscene / a movie from play.
pub fn dump_cutscene_facts() {
    use eldenring::cs::CSRemo;
    use fromsoftware_shared::FromStatic;
    let states = crate::inputs::ui_states();
    crate::erlog!("remo: interface elements created|visible<<1: {}", states.map_or("none".to_string(), |s| s.iter().enumerate().filter(|(_, v)| **v != 0).map(|(k, v)| format!("{k}:{v}")).collect::<Vec<_>>().join(" ")));
    // SAFETY: game thread; the singleton lives as long as the game; pointers are checked before they are followed.
    let Ok(remo) = (unsafe { CSRemo::instance() }) else {
        crate::erlog!("remo: no CSRemo");
        return;
    };
    let man = word!(remo.remo_man);
    if !plausible(man) {
        crate::erlog!("remo: no manager object");
        return;
    }
    for row in 0..12usize {
        // SAFETY: the manager object is larger than 0x180 bytes (its step machine and fields).
        let words: Vec<String> = (0..4).map(|k| format!("{:016x}", unsafe { std::ptr::read_unaligned((man + row * 0x20 + k * 8) as *const u64) })).collect();
        crate::erlog!("remo: +{:#05x}: {}", row * 0x20, words.join(" "));
    }
}

/// (AGGRO) THE GAME'S DEBUG CHARACTER CREATOR, at its place for the running version (`gamever::wcm`: the SDK's field
/// is the 2.7.1.0 place; in 2.7.2.0 it sits 0x10 further, so `wcm.debug_chr_creator` read 0 there and no ally body
/// could ever be made). Used only when the object's first word is CSDebugChrCreator's own vtable (RTTI, per version):
/// a wrong place gives `None`, never a write into another object.
pub(crate) fn debug_chr_creator() -> Option<&'static mut eldenring::cs::CSDebugChrCreator> {
    use crate::gamever::{pick, wcm};
    // SAFETY: game thread; the singleton lives as long as the game.
    let w = unsafe { WorldChrMan::instance() }.ok()? as *const WorldChrMan as usize;
    let at = pick(wcm::DEBUG_CHR_CREATOR.0, wcm::DEBUG_CHR_CREATOR.1)?;
    let vt = crate::entry::exe_base() + pick(wcm::DEBUG_CREATOR_VTABLE.0, wcm::DEBUG_CREATOR_VTABLE.1)?;
    let p = crate::probe::word(w + at).filter(|p| plausible(*p))?;
    (crate::probe::word(p) == Some(vt)).then(|| {
        // SAFETY: a live CSDebugChrCreator (its own vtable), owned by WorldChrMan for the game's life.
        unsafe { &mut *(p as *mut eldenring::cs::CSDebugChrCreator) }
    })
}

/// (AGGRO) The SDK's `WorldChrMan::spawn_debug_character` on [`debug_chr_creator`] (the SDK's method writes through
/// the 2.7.1.0 field place). `Err` when there is no creator.
pub(crate) fn spawn_debug_character(request: &eldenring::cs::ChrDebugSpawnRequest) -> Result<(), &'static str> {
    let c = debug_chr_creator().ok_or("the game has no debug character creator object")?;
    let mut name: Vec<u16> = format!("c{:0>4}", request.chr_id).encode_utf16().collect();
    name.resize(0x20, 0);
    c.init_data.name.clone_from_slice(&name);
    c.init_data.chara_init_param_id = request.chara_init_param_id;
    c.init_data.npc_param_id = request.npc_param_id;
    c.init_data.npc_think_param_id = request.npc_think_param_id;
    c.init_data.event_entity_id = request.event_entity_id;
    c.init_data.talk_id = request.talk_id;
    c.init_data.spawn_position = F32Vector4(request.pos_x, request.pos_y, request.pos_z, 0.0);
    c.spawn = true;
    Ok(())
}

/// (AGGRO) WorldChrMan's player camera object (`chr_cam`) at its place for the running version (`gamever::wcm`);
/// `None` on an unknown version or a null / implausible word.
pub(crate) fn chr_cam(wcm: &WorldChrMan) -> Option<std::ptr::NonNull<eldenring::cs::ChrCam>> {
    use crate::gamever::{pick, wcm as f};
    let at = pick(f::CHR_CAM.0, f::CHR_CAM.1)?;
    let p = crate::probe::word(wcm as *const WorldChrMan as usize + at).filter(|p| plausible(*p))?;
    std::ptr::NonNull::new(p as *mut eldenring::cs::ChrCam)
}
