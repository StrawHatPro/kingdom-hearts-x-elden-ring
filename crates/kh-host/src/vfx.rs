//! THE EFFECT SIDE OF A HOST, between `kh_core`'s effect seam and `kh_vfx`, without any drawing:
//! what a spawn of the core means (a notify names its particle system; a spell actor names its
//! blueprint class, whose body / launch effects come from the pack's `host.json`), where each
//! effect sits (a world point, Sora's sockets from the step's pose, the Keyblade's sockets, a
//! target, a projectile), the attach rows' placement (offset, scale, fade, follow), the Keyblade
//! trail, the Blizzard ice trail's pieces, one evaluator step per simulation step, and the draw
//! list of a frame. The host draws the list (Bevy meshes in the test app, Direct3D 12 in the
//! Elden Ring mod).
//!
//! THIS IS A PORT of the host-independent half of crates/kh-app/src/vfx.rs (its fifth pass,
//! 2026-10-12), made for the Elden Ring host: same tables, same rules, same order. The test app
//! still runs its own copy (its drawing is interleaved with it); a rule changed there must be
//! changed here too until the app is moved onto this module (IMPL_ER_FX_STATE.md, "remains").
//!
//! What a host gets asked for besides drawing: `EffectHost::take_shakes` (camera shakes, scaled
//! by the eye's distance) and `scene_light` (Thunder's scene darkening): both are REQUESTS; a
//! host that does not carry them out loses nothing else.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};

use glam::{Affine3A, Mat3, Vec2, Vec3};
use kh_core::{Attach, EffectHandle, EffectSpawn, Output, Sora, TargetId};
use kh_vfx::{DrawList, EffectDef, SpawnOptions, Xform};
use serde_json::Value;

use crate::audio::PoseTap;
use crate::seam::EffectCommand;

// (AUDIT-B F-SHIP) the form weapons' own bones as effect anchors (`kb:<model>:<mount>:<bone>`)
mod kbweapon;
// (FINISH-AUDIT2) an AttachEffect row's attach OBJECT (the weapon rows: on the weapon, not on his socket of that name)
pub mod weaponattach;
// (MIMIC-FX) the Sora instances (the Mimic Tears' Sora copies): their own effects on their own bones, Keyblade and form
mod instances;
pub use instances::{LivePlace, CORE_BITS};

/// A socket of Sora's skeleton or of the Keyblade: a bone and a relative transform (UE cm,
/// rotator degrees pitch / yaw / roll).
#[derive(Clone, Debug)]
struct Socket {
    bone: String,
    location: Vec3,
    rotation: Vec3,
}

/// Where and how an attach row places its effect (`SQEX_StructParticleAttachData`, spec 4.3).
#[derive(Clone, Copy, Debug)]
struct Placement {
    offset: Vec3,
    /// Pitch, yaw, roll.
    rotation: Vec3,
    scale: Vec3,
    /// `ToAttach`: it follows what it is attached to.
    follow: bool,
    absolute_rotation: bool,
    /// `AttachEffectEndType` FADEOUT: seconds.
    fade_out: Option<f32>,
    fade_in: f32,
    start_delay: f32,
    color: [f32; 4],
}

impl Default for Placement {
    fn default() -> Placement {
        Placement { offset: Vec3::ZERO, rotation: Vec3::ZERO, scale: Vec3::ONE, follow: true, absolute_rotation: false, fade_out: None, fade_in: 0.0, start_delay: 0.0, color: [1.0; 4] }
    }
}

#[derive(Clone, Debug)]
enum Anchor {
    World(Xform),
    /// A socket / bone of Sora (None = his mesh component), with the core's offset in actor space.
    Sora(Option<String>, Vec3),
    Target(TargetId, Vec3),
    /// The spell actor that owns this handle.
    Projectile,
    /// (ER party S5) A bone / socket of a party member (`kh_core::PartyMember.0`; None = the actor), with the core's
    /// offset in that frame: resolved on the member's latest pose (`EffectHost::set_party`).
    Party(u8, Option<String>, Vec3),
}

/// (ER party S5) THE PARTY'S SECOND ROOT: the material / mesh names of the effects read from `PACK/party/vfx` that the
/// main pack does not have carry this prefix on their way to a renderer, which then takes them from the party root
/// (`PACK/party/vfx`: its own `shaders/shading.json`, its own built shaders `er/`, its materials, meshes, textures).
/// The main pack's names never carry it, so the main shading table and its ids are the only ones Sora's effects see
/// (the party table's ids differ for 769 of the main materials: it must never stand in for the main one).
pub const PARTY_NAMESPACE: &str = "party:";

/// (ER party S5) One party member as of the latest step (KH3 space): the actor origin (feet) and facing, every bone
/// and socket of his pose (world position, rotation; the weapon's sockets as `wep:<name>`), the playing anim-set row.
#[derive(Clone, Debug, Default)]
pub struct PartyMemberFx {
    pub position: Vec3,
    pub yaw: f32,
    pub bones: HashMap<String, (Vec3, glam::Quat)>,
    pub anim: String,
}

impl PartyMemberFx {
    /// Where `Attach::Party { bone, offset }` is: the bone's (or the actor's) world transform with the offset in its
    /// own frame (kh-app party_fx `xform` / `point`, the same rule). A bone the skeleton lacks: a metre above his feet.
    pub fn xform(&self, bone: Option<&str>, offset: Vec3) -> (Vec3, glam::Quat) {
        let actor = (self.position, glam::Quat::from_rotation_z(self.yaw.to_radians()));
        let (p, r) = match bone {
            None => actor,
            Some(b) => self.bones.get(b).copied().unwrap_or((self.position + Vec3::Z * 100.0, actor.1)),
        };
        (p + r * offset, r)
    }
}

/// (ER90, XSI request 1) THE XS ABILITIES' ROOT: the material / mesh names of the effects read from `PACK/xsi/vfx` that
/// the main pack does not have carry this prefix on their way to a renderer, which takes them from that root (its own
/// `shaders/shading.json`, its own built shaders `er/`, its materials, meshes, textures): its table's ids never stand in for
/// the main or the party table's.
pub const XSI_NAMESPACE: &str = "xsi:";

/// (ER90) The XS moves' form effects (kh-app vfx.rs `xsi_allowed`, the same prefixes): the effect-group rule's name filter
/// lets them through once the XSI root is read (the core's group rule still decides which rows spawn).
const XSI_ALLOWED_PREFIXES: [&str; 5] = ["vw_so200_", "vw_so050_", "vw_so052_", "vw_so088_", "vw_so192_"];
/// (ER90) The XS moves' effects of the MAIN pack the name filter would drop: Spiral Rush's tube (M416, group 2100) and
/// Spectral Rays' beam and hit (abilities/xs/spectral.rs). (Myriad's Ultima summon vw_so191_ / vw_so192_: by prefix.)
/// (SHOT-A, INT-F) XSI's own beams are gone (Spectral Rays is KH3's laser family now, magic/shotlocks/laser.rs): its
/// effects are M_A481's notifies (build/gen/rust/sora_anim.rs, sequence A481) and the range's hit effect
/// (`laser.range.hit_effect`); they pass for the XS choice with a Keyblade other than Ever After (whose own pass anyway).
const XSI_ALLOWED_MAIN: [&str; 7] = ["vw_so200_spr_tub00", "vw_so072_stf_fir0", "vw_so072_stf_przm0", "vw_so072_stf_przm_one_set1", "vw_so072_stf_przm_set3", "vw_so072_wep_lgt0", "vw_so072_stf_hit0"];

/// (XW-d) `fx.xs_own` (default on; read before the XSI root is read): an XS effect that has a file in
/// `PACK/xsi/vfx/effects_xwd/` is read from there instead of `effects/` (the XS assets' own materials under the suffix
/// rule `name__<last 3 folders>` where the main pack has another asset of that name, the fragment pieces the root
/// lacked). Off: `effects/` only, as before.
pub static XS_OWN_FILES: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

/// (ER90) The XSI root's index (see `EffectHost::load_xsi_root`).
/// (P5 WEAPON DRAW) THE KEYBLADES' ROOT, `PACK/keyblades/vfx` (tools/sora_keyblades_pack.js): the effects of every keyblade
/// and its forms the other roots lack; its own materials / meshes named `kbfx:<name>` for the renderer (its own table there).
pub const KB_NAMESPACE: &str = "kbfx:";

/// (P5) The keyblades' root as read.
struct KbRoot {
    dir: PathBuf,
    /// Lower-case effect name -> its file name (only names the other roots lack).
    files: HashMap<String, String>,
    /// Lower-case names of the main pack's materials / meshes: they keep their main names and table (as the XSI root's; the
    /// keyblades' shading table was built so: tools/sora_vfx_shaders.js takes the main pack's file of a name both have)
    main_materials: HashSet<String>,
    main_meshes: HashSet<String>,
}

/// (P5) The keyblade in his hand, for the effect side (`EffectHost::set_keyblade`; None = the Kingdom Key's own).
#[derive(Default)]
struct KbFx {
    /// its trail (`kb.<KEY>.fx.trail`: what KH3's ead_key_trail0 plays for it)
    trail: Option<String>,
    /// its skeleton's sockets (vfx_top / vfx_bottom: its blade's own length), relative to its bone `wep`
    sockets: HashMap<String, Socket>,
    /// the lower-case name prefixes of ITS effects (`vw_so080`, its forms' models' ...): let through the other-keyblade rule
    allowed: Vec<String>,
}

struct XsiRoot {
    dir: PathBuf,
    /// Lower-case effect name -> the XSI root's file name (only names the main pack and the party root lack).
    files: HashMap<String, String>,
    main_materials: HashSet<String>,
    main_meshes: HashSet<String>,
    /// Lower-case names of the XS form effects the name filter lets through.
    allowed: HashSet<String>,
    /// (XW-d) Lower-case names of the effects read from `effects_xwd/` (XS_OWN_FILES).
    own: HashSet<String>,
}

/// (ER90) Gives a root's own material / mesh names (those the main pack lacks) the namespace.
fn namespace_def(def: &mut EffectDef, namespace: &str, main_materials: &HashSet<String>, main_meshes: &HashSet<String>) {
    let material = |name: &mut String| {
        if !name.is_empty() && !name.starts_with(namespace) && !main_materials.contains(&name.to_lowercase()) {
            *name = format!("{namespace}{name}");
        }
    };
    let mesh = |name: &mut String| {
        if !name.is_empty() && !name.starts_with(namespace) && !main_meshes.contains(&name.to_lowercase()) {
            *name = format!("{namespace}{name}");
        }
    };
    for e in &mut def.emitters {
        material(&mut e.material);
        if let Some(d) = e.decal.as_mut() {
            material(&mut d.material);
        }
        if let Some(m) = e.mesh.as_mut() {
            mesh(m);
        }
        if let Some(s) = e.sub_meshes.as_mut() {
            s.list.iter_mut().for_each(mesh);
        }
    }
}

/// (ER party S5) The second root's index (see `EffectHost::load_party_root`).
struct PartyRoot {
    dir: PathBuf,
    /// Lower-case effect name -> the party root's file name (only names the main pack lacks).
    files: HashMap<String, String>,
    /// Lower-case names of the main pack's materials / meshes (they keep their main names and table).
    main_materials: HashSet<String>,
    main_meshes: HashSet<String>,
    /// Effect files of the party root registered so far.
    registered: usize,
}

struct Live {
    handle: EffectHandle,
    id: kh_vfx::EffectId,
    anchor: Anchor,
    placement: Placement,
    /// The Keyblade trail: fed with the two blade sockets every step.
    trail: bool,
    /// An endless effect a one-shot event started: ended when his animation changes.
    ends_with_animation: bool,
    /// The core asked it to end: a trail takes no more samples.
    stopped: bool,
    persistent: bool,
    /// (ER party S5) A party member's endless one-shot effect: the member and the anim-set row it was spawned in (it
    /// ends when his row changes, as Sora's do with his animation).
    party_anim: Option<(u8, String)>,
    /// (MIMIC-FX) The Sora instance (his party member id) it belongs to: placed on HIS scene by his own step
    /// (vfx/instances.rs); None: the player's Sora's, the party's, a host's own.
    inst: Option<u8>,
    /// (FINISH-AUDIT2) The effect file and the socket kh-core sent for it ("None": his mesh), for one of his sockets: the
    /// placement measurement's key (weaponattach.rs `measure_placements`), and the anchor KH3's row asked for at the spawn;
    /// None: any other anchor.
    sent: Option<(String, String, String)>,
    /// (FREEZE-FX) The character a PAWN'S OWN bad-state row belongs to when it is spawned in the world (ToAttach false): KH3's
    /// bad-state helper (0x1405B7C50 / 0x1405B77B0, `m_CmnBadStatesEffect`, attach mode "component owner") gives the component
    /// its owner's mesh, so the row's material drives reach that character (the frozen look's "Ice": Sora, a party member;
    /// never an enemy) and its particles that start on the mesh start on that character's bones. None: its anchor says.
    owner: Option<Anchor>,
    /// (FREEZE-FX) A row of the freeze groups (0xC1C .. 0xC20): KH3's SetFrozen (0x1405B77B0) sends the same group to the pawn's
    /// equipment component (`MyEquipment` +0x200 vt+0x1C8(2, 0xC1C) / vt+0x1A8(0xC1C, 1)), whose weapons' own
    /// m_CmnBadStatesEffect (ead_com_abn_w0, w_so_KeybladeBase) plays vs_abn_frz_in / vs_abn_frz_out0 on the WEAPON: its drives
    /// reach the held weapons' materials too (`CharDrive::equipment`).
    equipment: bool,
}

/// (FREEZE-FX) A body's bones by name: world place and turn (KH3 space, cm).
pub type BoneMap = HashMap<String, (Vec3, glam::Quat)>;

/// (FREEZE-FX) A row of the pawns' common bad-state asset `ead_com_abn0` (`m_CmnBadStatesEffect`), by the group id the spawn
/// carries (kh-core `combat::badstat::effect`; the frozen look's 0xC1C .. 0xC20).
fn pawn_bad_state_row(id: u32) -> bool {
    use kh_core::combat::badstat::effect;
    matches!(id, effect::SNEEZE | effect::GENERIC | effect::BURN | effect::STUN | effect::STOP) || (effect::ICE..=effect::ICE + 4).contains(&id)
}

/// `host.json` actors.TresRailSlideActor: the Blizzard ice trail's meshes and class defaults.
struct RailActor {
    /// (mesh, material, length of the mesh along its X in cm).
    segments: Vec<(String, String, f32)>,
    scale: Vec2,
    scale_random: (Vec2, Vec2),
    offset: f32,
    fade_wait: f32,
}

struct RailPiece {
    from: Vec3,
    to: Vec3,
    segment: usize,
    scale: Vec2,
    born: f32,
}

/// Seconds a rail node (and so a trail piece) lives: `m_GenerateRailLife` (magic spec 4.2).
const RAIL_LIFE: f32 = 3.0;

/// What the effect side needs to know of the world it is in, each step.
pub struct Scene<'a> {
    /// The step's output (position, yaw, the animation's play id).
    pub out: &'a Output,
    /// The core (its live spell actors).
    pub sora: &'a Sora,
    /// The bones of the step's pose (empty: no pose; effects then sit a metre above his origin).
    pub tap: &'a PoseTap,
    /// The centre of a target (its origin + half its height), cm.
    pub target: &'a dyn Fn(TargetId) -> Option<Vec3>,
    /// The height of the ground for the emitters that cast a ray to find it (Thunder's marks).
    pub ground: Option<f32>,
    /// The eye, for the camera shakes' distance scale.
    pub eye: Vec3,
}

/// (DON-KIT) The effects of the Recover kinds (the items' effects on whoever uses them: KH3's ItemEffect -> Recover table,
/// kh-core `recover_kind_effect`) and the party Cure's body (kh-party actions `recover_effect` 14 / 15): the names a host
/// makes ready before the first item use.
/// (AUDIT-A STAFF) The mesh component of FINISH_WA's tower (`kb:w_so073:tower:<bone>`: TresStylePawnWATower, its own actor
/// at the root and yaw Sora had in StyleFinishWA Tick sub 0, kh-core `staff::tower`; TresCharMesh0 RelativeRotation yaw 270
/// as his), whose bones its own A417 poses in step with his (both start in that tick). None: no tower.
fn tower_component(s: &Scene) -> Option<Xform> {
    kh_core::combat::forms::staff::tower(s.sora.finish(), s.sora.combat()).map(|t| Xform::from_rotator(t.position, 0.0, t.yaw - 90.0, 0.0, 1.0))
}

pub fn item_effect_names() -> Vec<&'static str> {
    let mut names: Vec<&'static str> = (0u8..=0x40).filter_map(kh_core::host::audio::recover_kind_effect).map(|r| r.0).collect();
    names.push("vm_cur_bds1");
    names.sort_unstable();
    names.dedup();
    names
}

/// What a material is drawn on (for a renderer's preloading).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AssetGeo {
    Quad,
    Ribbon,
    Mesh,
    Decal,
}

/// A camera shake to play: its class (`host.json` camera_shakes), scale and duration.
#[derive(Clone, Debug, PartialEq)]
pub struct Shake {
    pub class: String,
    pub scale: f32,
    pub duration: Option<f32>,
}

pub struct EffectHost {
    dir: PathBuf,
    pub world: kh_vfx::World,
    sockets: HashMap<String, Socket>,
    weapon_sockets: HashMap<String, Socket>,
    /// Projectile blueprint class -> role -> effects.
    projectiles: HashMap<String, HashMap<String, Vec<String>>>,
    /// (effect, socket) -> placement, from the trigger table.
    placements: HashMap<(String, String), Placement>,
    /// Effect -> the socket of its first attach row (for a spawn "on Sora" without a socket).
    default_sockets: HashMap<String, String>,
    /// Lower-case effect name -> the pack's file name.
    files: HashMap<String, String>,
    tried: HashSet<String>,
    live: Vec<Live>,
    rail: Option<RailActor>,
    rail_last: Option<(Vec3, f32)>,
    rail_pieces: Vec<RailPiece>,
    cure_group: u32,
    last_handle: u32,
    play_id: u32,
    seconds: f32,
    shakes: Vec<Shake>,
    list: DrawList,
    /// The names behind the draw list's material and mesh ids (index = id).
    material_names: Vec<String>,
    mesh_names: Vec<String>,
    pub spawned: u64,
    /// Lines for the host's log (each said once).
    said: HashSet<String>,
    pub log: Vec<String>,
    /// The effects of Sora's own triggers and spell actors (the preload list), by file name.
    wanted: Vec<String>,
    /// (prize / savepoint) The free effects that were started (`free_spawn`): `clear` stops them too.
    free: Vec<u64>,
    /// (IMPL_ER53, PORT ADDITION) Copies of the pack's effects a host defines (`define_variant`): name -> (base effect,
    /// kept emitters, held clock). Registered on first use; placed as their base.
    variants: HashMap<String, (String, Vec<String>, Option<f32>)>,
    /// (IMPL_ER75) the Keyblade trails since the last read (`take_trail_stats`): steps with a live trail, the largest
    /// distance between the trail's newest sample and the blade's tip socket after the step (cm), the most samples, the
    /// effect clock's step (s), the cuts.
    pub trail_stats: TrailStats,
    /// (IMPL_ER75, a HOST CHOICE against KH3's data) the melee swing arcs (`vp_*_sho*`: their attach rows say `ToAttach`
    /// false, spawned in the world where the swing starts) follow Sora instead: in a fast dash they were left behind him
    /// (45.jpg). false = KH3's rows.
    pub slash_follow: bool,
    /// (AUDIT-B W-FIN) The `EffectCommand::Timing` that follows the spawn being started (`step` looks it up): the row's own
    /// FadeIn / StartDelayTime / FadeOut, over the placement table's.
    timing_next: Option<kh_core::host::EffectTiming>,
    /// (ER party S5) The party's second root (None until `load_party_root`: everything as before).
    party_root: Option<PartyRoot>,
    /// (ER party S5) The members as of the latest step, and the party's spell actors / team props (body-effect handle,
    /// position, flight direction).
    party_members: HashMap<u8, PartyMemberFx>,
    party_projectiles: Vec<(u32, Vec3, Vec3)>,
    /// (FREEZE-FX) The targets' bones the host could read (world place and turn, cm): the effects on a target whose particles
    /// start on its mesh (KH3's frozen look) spawn on them. A target missing here: they start at its anchor.
    target_bones: HashMap<u32, std::sync::Arc<BoneMap>>,
    /// (FREEZE-FX) UseLastParameter drives (kh_vfx `CharParam::keep`) of persistent spawns not stopped yet: KH3 leaves the last
    /// value on the mesh after the emitter ends (the frozen look's "Ice" held for the whole freeze): (the spawn's handle, the
    /// drive as last resolved). Ended by the handle's stop, or by a newer drive of the same owner, parameter and mesh.
    kept_drives: Vec<(EffectHandle, CharDrive)>,
    /// (ER90) The XS abilities' root (None until `load_xsi_root`: everything as before).
    xsi_root: Option<XsiRoot>,
    /// (P5) The keyblades' root (None until `load_kb_root`) and the keyblade in his hand (None: the Kingdom Key).
    kb_root: Option<KbRoot>,
    kb: Option<KbFx>,
    /// (LOADIN) Effect names to parse ahead of their first spawn (`queue_preload`), a few per `preload_step`.
    preload_queue: std::collections::VecDeque<String>,
    /// (LOADIN) Every name ever queued (lower case): queued once.
    preload_seen: HashSet<String>,
    /// (INT) His place, facing and bones as of the step BEFORE the latest (an effect spawned on one of his sockets is
    /// drawn from where that socket was then: `World::set_previous_transform`).
    int_before: Option<(Output, PoseTap)>,
    /// (AUDIT-B F-SHIP) The form weapons' / pawns' own skeletons and clips for the `kb:` anchors (vfx/kbweapon.rs).
    kb_rigs: kbweapon::KbRigs,
    /// (HAIR-PULSE) The model of his right-hand weapon this step (the held form's `RWeapon` unit at its grow level,
    /// `Form::weapon_units`: w_so200, w_so010 ...; None out of a form): where a SUB in-light drive of an effect on his own
    /// sockets goes (KH3: the form charge glow vs_com_lv2 hangs on that weapon, `char_drives`).
    right_weapon: Option<String>,
    /// (MIMIC-FX) The Sora instances by party member id (their own Sora facts: vfx/instances.rs), and the one whose step
    /// runs now (the effects it starts are his).
    instances: HashMap<u8, instances::SoraInst>,
    inst_now: Option<u8>,
}

/// (AUDIT-B F-SHIP) KH3's `ead_key_trail0` row of a form clip (Highwind's M_E412 on R_wep5_skt, Storm Flag's on R_wep6_skt):
/// the weapon actor on that socket plays its DesignatedAttachData `ead_key_trail0`, whose row asks for the weapon's common
/// `key_trail0` (w_so092_Wep / w_so093_Wep TresEffectAttach0: CommonEffectList key_trail0 = vw_so090_loc0, the hand
/// keyblade's trail): the trail on THAT weapon's vfx_top / vfx_bottom. The anchor of its tip, None when the spawn is not
/// such a row or no unit of the held form stands on that socket.
fn form_trail_anchor(spawn: &EffectSpawn, s: &Scene) -> Option<String> {
    if !kh_vfx::base_name(spawn.name).eq_ignore_ascii_case("ead_key_trail0") {
        return None;
    }
    if !matches!(spawn.at, Attach::Sora { .. }) {
        return None;
    }
    // The rows are SQEX_ATTACH_OBJECT_ATTACH_TO_RIGHT_HAND_WEAPON (their SocketName R_wep5_skt / R_wep_skt is not where it
    // goes): the held form's RIGHT_HAND unit, on its own socket (`RWeapon@RIGHT_HAND:R_wep5_skt=.../w_so092_Wep`).
    let form = &s.sora.combat().form;
    let units = form.weapon_units();
    let unit = units.iter().find(|u| u.contains("@RIGHT_HAND:"))?;
    let socket = unit.split_once("@RIGHT_HAND:").and_then(|(_, rest)| rest.split('=').next()).filter(|s| !s.is_empty() && *s != "None").unwrap_or("R_wep_skt");
    let model = kh_core::status::keyblade::model_of(kh_core::status::keyblade::blueprint_of(unit));
    Some(format!("kb:{model}:{socket}:vfx_top"))
}

/// (AUDIT-C SW) An AttachEffect notify entry naming an ATTACH DATA ASSET of a form's clips (Ultima's ead_so192_aura0, on every
/// AnimSet9 clip; kh-core `ultimate::attach_rows`): KH3 plays the asset's rows (group 0, the always-group: the asset has no
/// other), each where ITS row attaches it (SQEX_StructParticleAttachData): an OWNER row on Sora's socket (its placement is in
/// the table, `EffectHost::load`), an ATTACH_TO_RIGHT_HAND_WEAPON row on the right-hand weapon's socket: the held form's
/// RIGHT_HAND unit (`kb:<model>:<mount>:<socket>`, as `form_trail_anchor`), else the Keyblade's own socket. None: not such an
/// asset (every other spawn resolves as before).
fn attach_data_rows(spawn: &EffectSpawn, s: &Scene) -> Option<Vec<(String, Anchor, bool)>> {
    let Attach::Sora { offset, .. } = spawn.at else { return None };
    let rows = kh_core::combat::forms::ultimate::attach_rows(kh_vfx::base_name(spawn.name), 0);
    if rows.is_empty() {
        return None;
    }
    let units = s.sora.combat().form.weapon_units();
    let right = units.iter().find(|u| u.contains("@RIGHT_HAND:")).map(|unit| {
        let mount = unit.split_once("@RIGHT_HAND:").and_then(|(_, rest)| rest.split('=').next()).filter(|m| !m.is_empty() && *m != "None").unwrap_or("R_wep_skt");
        (kh_core::status::keyblade::model_of(kh_core::status::keyblade::blueprint_of(unit)), mount)
    });
    Some(
        rows.iter()
            .map(|r| {
                let socket = match (r.right_hand_weapon, right) {
                    (true, Some((model, mount))) => format!("kb:{model}:{mount}:{}", r.socket),
                    _ => r.socket.to_string(),
                };
                (r.system.to_string(), Anchor::Sora(Some(socket), offset), false)
            })
            .collect(),
    )
}

/// (AUDIT-C TF) Double Form's attach data asset of its clips (AnimSet23 AirDodge N054: vw_so220_warp, kh-core
/// `lightdark::attach_rows`): KH3 plays the asset's rows of the request's group, each on ITS socket with ITS offset / scale
/// (SQEX_StructParticleAttachData; every row is an OWNER row: Sora). The socket carries the row (`center:ead11`): its
/// placement is keyed by it (`EffectHost::load`), the flag itself is dropped where a socket's flags are read. None: not such
/// an asset.
fn tf_attach_rows(spawn: &EffectSpawn) -> Option<Vec<(String, Anchor, bool)>> {
    let Attach::Sora { offset, .. } = spawn.at else { return None };
    let rows = kh_core::combat::forms::lightdark::attach_rows(kh_vfx::base_name(spawn.name), 0); // (group 0: the always-group, the only one of vw_so220_warp; a notify spawn carries its asset id, not a group)
    (!rows.is_empty()).then(|| rows.iter().map(|r| (r.system.to_string(), Anchor::Sora(Some(tf_row_socket(r)), offset), false)).collect())
}

/// (AUDIT-C TF) The socket of an attach data row with its row flag (`center:ead11`).
fn tf_row_socket(r: &kh_core::combat::forms::lightdark::AttachRow) -> String {
    format!("{}:ead{}", r.socket, r.index)
}

/// (IMPL_ER75) A melee swing arc of the pack (`vp_qst_sho00`, `vp_fst_sho01`, `vp_rad_sho00` ...).
pub fn is_slash_arc(file: &str) -> bool {
    let lower = file.to_lowercase();
    lower.starts_with("vp_") && lower.contains("_sho")
}

/// (KBFX) A four-digit weapon group (`vw_so0033_lv1`) as its three-digit model's group (`vw_so033_lv1`); `None` for
/// any other name.
fn four_digit_group(lower: &str) -> Option<String> {
    let digits = lower.strip_prefix("vw_so0")?;
    let (number, rest) = digits.split_at_checked(3)?;
    (number.bytes().all(|b| b.is_ascii_digit()) && rest.starts_with('_')).then(|| format!("vw_so{number}{rest}"))
}

/// (IMPL_ER75) See `EffectHost::trail_stats`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TrailStats {
    pub steps: u32,
    pub max_gap: f32,
    pub max_samples: usize,
    pub dt: f32,
    pub cuts: u32,
}

fn vec3_of(v: Option<&Value>, default: Vec3) -> Vec3 {
    match v {
        Some(Value::Array(a)) if a.len() >= 3 => Vec3::new(a[0].as_f64().unwrap_or(0.0) as f32, a[1].as_f64().unwrap_or(0.0) as f32, a[2].as_f64().unwrap_or(0.0) as f32),
        Some(Value::Object(o)) if o.contains_key("x") => Vec3::new(o["x"].as_f64().unwrap_or(0.0) as f32, o["y"].as_f64().unwrap_or(0.0) as f32, o["z"].as_f64().unwrap_or(0.0) as f32),
        Some(Value::Object(o)) if o.contains_key("pitch") => Vec3::new(o["pitch"].as_f64().unwrap_or(0.0) as f32, o["yaw"].as_f64().unwrap_or(0.0) as f32, o["roll"].as_f64().unwrap_or(0.0) as f32),
        _ => default,
    }
}

/// (SHOT-FIX) The attach data of the AttachEffect notifies of every clip of Sora's (kh_core's generated `SEQUENCES`:
/// each notify's `MetaAttachDataList`, the same `SQEX_StructParticleAttachData` fields the triggers rows carry):
/// `SocketName`, `LocationOffset`, `RotationOffset`, `ScaleOffset`, `ToAttach`, `bAbsoluteRotation`, `FadeIn`, `FadeOut`
/// with `AttachEffectEndType` FADEOUT, `StartDelayTime`, `ParticleColor` / `ParticleAlpha`, per effect of its
/// `EffectDataList`. The triggers table (sheets/port/sora-effects.json) lists only the clips of p_ex001_AnimSet; the
/// forms' and the shotlocks' clips (e.g. Atomic Deluge's M_B481: vw_so033_gsh_spk01 on vfx_root, 245 cm up, absolute
/// rotation) got no placement before and were drawn on the bare socket in its bone's rotation. These rows fill only the
/// (effect, socket) pairs the triggers table does not place.
fn notify_placements() -> Vec<((String, String), Placement)> {
    let mut out = Vec::new();
    for sequence in kh_core::anim::SEQUENCES.iter() { // (PACKGEN) a run-time table in the release build (kh-gendata)
        for notify in sequence.notifies {
            let Some((_, json)) = notify.props.iter().find(|(name, _)| *name == "MetaAttachDataList") else { continue };
            let Ok(Value::Array(list)) = serde_json::from_str::<Value>(json) else { continue };
            for meta in list.iter().filter_map(|m| m.get("MetaAttachData").and_then(Value::as_array)).flatten() {
                let Some(socket) = meta.get("SocketName").and_then(Value::as_str) else { continue };
                let num = |key: &str| meta.get(key).and_then(Value::as_f64).unwrap_or(0.0) as f32;
                let color = vec3_of(meta.get("ParticleColor"), Vec3::ONE);
                let placement = Placement {
                    offset: vec3_of(meta.get("LocationOffset"), Vec3::ZERO),
                    rotation: vec3_of(meta.get("RotationOffset"), Vec3::ZERO),
                    scale: vec3_of(meta.get("ScaleOffset"), Vec3::ONE),
                    follow: meta.get("ToAttach").and_then(Value::as_bool).unwrap_or(true),
                    absolute_rotation: meta.get("bAbsoluteRotation").and_then(Value::as_bool).unwrap_or(false),
                    fade_out: meta.get("AttachEffectEndType").and_then(Value::as_str).is_some_and(|t| t.ends_with("FADEOUT")).then(|| num("FadeOut")),
                    fade_in: num("FadeIn"),
                    start_delay: num("StartDelayTime"),
                    color: [color.x, color.y, color.z, meta.get("ParticleAlpha").and_then(Value::as_f64).unwrap_or(1.0) as f32],
                };
                for effect in meta.get("EffectDataList").and_then(Value::as_array).into_iter().flatten().filter_map(|e| e.get("AttachEffect").and_then(Value::as_str)) {
                    out.push(((kh_vfx::base_name(effect).to_lowercase(), socket.to_string()), placement));
                }
            }
        }
    }
    out
}

impl EffectHost {
    /// Reads the host table and the placement rows of `PACK/vfx`. No effect is parsed yet:
    /// `preload` (on any thread, before the host is used) or the first spawn does that.
    pub fn load(pack_dir: &Path) -> Result<EffectHost, String> {
        let dir = pack_dir.join("vfx");
        if !dir.join("effects").is_dir() {
            return Err(format!("no effect pack at {} (node tools/sora_vfx_pack.js)", dir.display()));
        }
        let mut host = EffectHost {
            dir: dir.clone(),
            world: kh_vfx::World::new(),
            sockets: HashMap::new(),
            weapon_sockets: HashMap::new(),
            projectiles: HashMap::new(),
            placements: HashMap::new(),
            default_sockets: HashMap::new(),
            files: HashMap::new(),
            tried: HashSet::new(),
            live: Vec::new(),
            rail: None,
            rail_last: None,
            rail_pieces: Vec::new(),
            cure_group: 1424,
            last_handle: 0,
            play_id: 0,
            seconds: 0.0,
            shakes: Vec::new(),
            list: DrawList::default(),
            material_names: Vec::new(),
            mesh_names: Vec::new(),
            spawned: 0,
            said: HashSet::new(),
            log: Vec::new(),
            wanted: Vec::new(),
            free: Vec::new(),
            variants: HashMap::new(),
            trail_stats: TrailStats::default(),
            slash_follow: false,
            timing_next: None, // (AUDIT-B W-FIN)
            party_root: None,
            party_members: HashMap::new(),
            party_projectiles: Vec::new(),
            target_bones: HashMap::new(), // (FREEZE-FX)
            kept_drives: Vec::new(), // (FREEZE-FX)
            xsi_root: None,
            kb_root: None,
            kb: None,
            preload_queue: std::collections::VecDeque::new(), // (LOADIN)
            preload_seen: HashSet::new(),
            int_before: None,
            kb_rigs: kbweapon::KbRigs::new(pack_dir.to_path_buf()), // (AUDIT-B F-SHIP)
            right_weapon: None, // (HAIR-PULSE)
            instances: HashMap::new(), // (MIMIC-FX)
            inst_now: None,
        };
        if let Ok(entries) = std::fs::read_dir(dir.join("effects")) {
            for entry in entries.flatten() {
                if let Some(name) = entry.file_name().to_str().and_then(|n| n.strip_suffix(".json")) {
                    host.files.insert(name.to_lowercase(), name.to_string());
                }
            }
        }
        let read = |file: &str| std::fs::read_to_string(dir.join(file)).ok().and_then(|text| serde_json::from_str::<Value>(&text).ok());
        match read("host.json") {
            Some(table) => {
                let sockets = |v: Option<&Value>, location: &str, rotation: &str| -> HashMap<String, Socket> { v.and_then(Value::as_object).map(|m| m.iter().map(|(name, s)| (name.clone(), Socket { bone: s["bone"].as_str().unwrap_or("").to_string(), location: vec3_of(s.get(location), Vec3::ZERO), rotation: vec3_of(s.get(rotation), Vec3::ZERO) })).collect()).unwrap_or_default() };
                host.sockets = sockets(table.get("sockets"), "location", "rotation");
                host.weapon_sockets = sockets(table.get("weapon_sockets"), "local_location", "local_rotation");
                if let Some(a) = table.get("actors").and_then(|a| a.get("TresRailSlideActor")) {
                    let pair = |v: Option<&Value>| Vec2::new(v.and_then(|v| v.get(0)).and_then(Value::as_f64).unwrap_or(0.0) as f32, v.and_then(|v| v.get(1)).and_then(Value::as_f64).unwrap_or(0.0) as f32);
                    let segments: Vec<(String, String, f32)> = a
                        .get("segments")
                        .and_then(Value::as_array)
                        .map(|list| {
                            list.iter()
                                .filter_map(|m| {
                                    let b = m.get("bounds_m")?;
                                    // The pack's bounds are in its glTF space (x, up, y) in metres; the mesh's length is along X.
                                    let length = (b.get("max")?.get(0)?.as_f64()? - b.get("min")?.get(0)?.as_f64()?) as f32 * 100.0;
                                    Some((m.get("mesh")?.as_str()?.to_string(), m.get("material")?.as_str()?.to_string(), length))
                                })
                                .collect()
                        })
                        .unwrap_or_default();
                    if !segments.is_empty() {
                        host.rail = Some(RailActor { segments, scale: pair(a.get("scale")), scale_random: (pair(a.get("scale_random_min")), pair(a.get("scale_random_max"))), offset: a.get("offset").and_then(Value::as_f64).unwrap_or(0.0) as f32, fade_wait: a.get("mesh_fade_out_wait").and_then(Value::as_f64).unwrap_or(2.6) as f32 });
                    }
                }
                if let Some(classes) = table.get("projectiles").and_then(Value::as_object) {
                    for (class, roles) in classes {
                        let roles = roles.as_object().map(|m| m.iter().map(|(role, list)| (role.clone(), list.as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect()).unwrap_or_default())).collect()).unwrap_or_default();
                        host.projectiles.insert(class.clone(), roles);
                    }
                }
            }
            None => host.say("vfx: no PACK/vfx/host.json (node tools/sora_vfx_host.js): sockets and spell effects are unknown, effects sit at his origin".to_string()),
        }
        let mut wanted: BTreeSet<String> = BTreeSet::new();
        if let Some(rows) = read("triggers.json").as_ref().and_then(|t| t.get("rows")).and_then(Value::as_array) {
            for row in rows {
                let Some(effect) = row.get("effect").and_then(Value::as_str) else { continue };
                if matches!(row.get("kk_base").and_then(Value::as_str), Some("yes") | Some("conditional")) {
                    wanted.insert(kh_vfx::base_name(effect).to_lowercase());
                }
                let Some(attach) = row.get("attach") else { continue };
                let Some(socket) = attach.get("socket").and_then(Value::as_str) else { continue };
                let params = row.get("params").unwrap_or(&Value::Null);
                let color = vec3_of(params.get("color"), Vec3::ONE);
                let fade_out = (params.get("end_type").and_then(Value::as_str) == Some("FADEOUT")).then(|| params.get("fade_out").and_then(Value::as_f64).unwrap_or(0.0) as f32);
                let placement = Placement {
                    offset: vec3_of(attach.get("offset"), Vec3::ZERO),
                    rotation: vec3_of(attach.get("rotation"), Vec3::ZERO),
                    scale: vec3_of(attach.get("scale"), Vec3::ONE),
                    follow: attach.get("to_attach").and_then(Value::as_bool).unwrap_or(true),
                    absolute_rotation: attach.get("absolute").and_then(|a| a.get("rotation")).and_then(Value::as_bool).unwrap_or(false),
                    fade_out,
                    fade_in: params.get("fade_in").and_then(Value::as_f64).unwrap_or(0.0) as f32,
                    start_delay: params.get("start_delay").and_then(Value::as_f64).unwrap_or(0.0) as f32,
                    color: [color.x, color.y, color.z, params.get("alpha").and_then(Value::as_f64).unwrap_or(1.0) as f32],
                };
                host.placements.entry((kh_vfx::base_name(effect).to_lowercase(), socket.to_string())).or_insert(placement);
                host.default_sockets.entry(kh_vfx::base_name(effect).to_lowercase()).or_insert(socket.to_string());
            }
        }
        // (SHOT-FIX) the clips the triggers table leaves out: their notifies' own attach data
        for (key, placement) in notify_placements() {
            host.placements.entry(key).or_insert(placement);
        }
        // (AUDIT-C SW) the rows of the form clips' attach data assets on Sora's own sockets (Ultima's ead_so192_aura0): each
        // row's own SQEX_StructParticleAttachData (the weapon's rows are placed by their `kb:` anchor)
        for r in kh_core::combat::forms::ultimate::all_attach_rows().into_iter().filter(|r| !r.right_hand_weapon) {
            let placement = Placement { offset: r.offset, rotation: r.rotation, scale: r.scale, follow: r.follow, absolute_rotation: r.absolute_rotation, fade_out: None, fade_in: r.fade_in, start_delay: r.start_delay, color: r.color };
            host.placements.entry((r.system.to_lowercase(), r.socket.to_string())).or_insert(placement);
        }
        // (AUDIT-C TF) Double Form's attach data rows (vw_so220_warp), each keyed by its socket and its row ("center:ead11"):
        // two rows of one system on one socket carry other offsets (bra0 on center at 0 and at y 50)
        for asset in kh_core::combat::forms::lightdark::attach_assets() {
            for r in kh_core::combat::forms::lightdark::attach_rows(asset, 0) {
                let placement = Placement { offset: r.offset, rotation: r.rotation, scale: r.scale, follow: r.follow, absolute_rotation: r.absolute_rotation, fade_out: None, fade_in: r.fade_in, start_delay: r.start_delay, color: r.color };
                host.placements.entry((r.system.to_lowercase(), tf_row_socket(&r))).or_insert(placement);
            }
        }
        for roles in host.projectiles.values() {
            for list in roles.values() {
                wanted.extend(list.iter().map(|n| n.to_lowercase()));
            }
        }
        // (DON-KIT) THE ITEM EFFECTS (the Recover kinds: Potion, Hi-Potion, Ether, Elixir ... and the party Cure's body) are
        // preloaded too: the user, 2026-10-22, "the little item VFX doesn't appear when Goofy and Donald first use an item"
        // (their first spawn waited on its materials while the one-second effect ran out)
        wanted.extend(item_effect_names().into_iter().map(str::to_lowercase));
        // (F-RAGE fix) RAGE FORM'S EFFECTS (the core spawns them by name, no trigger names them): preloaded too, so the aura's
        // materials are made with the pack and it shows from the transformation on (the user, 2026-11-03: "rage form doesn't
        // have the rage form particles")
        wanted.extend(kh_core::combat::forms::rage::effect_names().into_iter().map(str::to_lowercase));
        host.wanted = wanted.into_iter().filter_map(|n| host.files.get(&n).cloned()).collect();
        Ok(host)
    }

    /// One line about what was read.
    pub fn describe(&self) -> String {
        format!("effect pack {}: {} effects ({} of Sora's own to preload), {} sockets, {} weapon sockets, {} spell actor classes, {} placement rows", self.dir.display(), self.files.len(), self.wanted.len(), self.sockets.len(), self.weapon_sockets.len(), self.projectiles.len(), self.placements.len())
    }

    /// Parses every effect Sora's own triggers and spell actors name, with the systems nested in
    /// them (1 - 4 s: call it on a thread that may take that long). Returns how many are registered.
    pub fn preload(&mut self) -> usize {
        for file in self.wanted.clone() {
            self.ensure(&file);
        }
        self.tried.len()
    }

    /// Every (material, what it is drawn on, the emitter's blend) and mesh the registered effects
    /// name: what a renderer makes ready beforehand.
    pub fn assets(&self) -> (Vec<(String, AssetGeo, kh_vfx::Blend)>, Vec<String>) {
        let (mut materials, mut meshes): (Vec<(String, AssetGeo, kh_vfx::Blend)>, BTreeSet<String>) = (Vec::new(), BTreeSet::new());
        let mut add = |name: &str, geo: AssetGeo, blend: kh_vfx::Blend| {
            if !name.is_empty() && !materials.iter().any(|m| m.0 == name && m.1 == geo && m.2 == blend) {
                materials.push((name.to_string(), geo, blend));
            }
        };
        let mut files: Vec<&String> = self.tried.iter().collect();
        files.sort();
        for file in files {
            let Some(def) = self.world.def(file) else { continue };
            for e in &def.emitters {
                match e.kind {
                    kh_vfx::Kind::Sprite | kh_vfx::Kind::TailSprites => add(&e.material, AssetGeo::Quad, e.blend),
                    kh_vfx::Kind::Mesh => add(&e.material, AssetGeo::Mesh, e.blend),
                    // (VFX-2) a skeletal mesh component is drawn with its material's mesh-particle shaders
                    kh_vfx::Kind::Skeletal if e.skeletal.as_ref().is_some_and(|s| s.override_material) => add(&e.material, AssetGeo::Mesh, e.blend),
                    kh_vfx::Kind::AnimTrail | kh_vfx::Kind::Tail | kh_vfx::Kind::Beam /* (AUDIT-B W-VFX) a beam is a strip */ | kh_vfx::Kind::DynamicShape /* (VFX-RING S) strips */ => add(&e.material, AssetGeo::Ribbon, e.blend),
                    _ => {}
                }
                if let Some(d) = &e.decal {
                    add(&d.material, AssetGeo::Decal, kh_vfx::Blend::Material);
                }
                if let Some(mesh) = &e.mesh {
                    meshes.insert(mesh.clone());
                }
                for mesh in e.sub_meshes.iter().flat_map(|s| s.list.iter()) {
                    meshes.insert(mesh.clone());
                }
            }
        }
        if let Some(rail) = &self.rail {
            for (mesh, material, _) in &rail.segments {
                meshes.insert(mesh.clone());
                add(material, AssetGeo::Mesh, kh_vfx::Blend::Material);
            }
        }
        (materials, meshes.into_iter().collect())
    }

    fn say(&mut self, line: String) {
        if self.said.insert(line.clone()) && self.log.len() < 256 {
            self.log.push(line);
        }
    }

    /// (IMPL_ER53, PORT ADDITION) A copy of the pack's effect `base` spawnable as `name` (`BASE#something`: the part
    /// before `#` names the base, whose placement rows it takes): only the emitters in `keep` enabled, its clock held at
    /// `hold` seconds (`None`: runs as authored). Defining it again with other values takes effect at its next spawn.
    pub fn define_variant(&mut self, name: &str, base: &str, keep: &[&str], hold: Option<f32>) {
        let wanted = (base.to_string(), keep.iter().map(|k| k.to_string()).collect::<Vec<_>>(), hold);
        if self.variants.get(name) != Some(&wanted) {
            self.variants.insert(name.to_string(), wanted);
            // (made again from the base at its next use)
            self.tried.remove(name);
        }
    }

    /// Registers an effect of the pack with the evaluator (once); `None` when there is none.
    fn ensure(&mut self, name: &str) -> Option<String> {
        if let Some((base, keep, hold)) = self.variants.get(name).cloned() {
            if self.tried.insert(name.to_string()) {
                let made = self.ensure(&base).and_then(|file| self.world.def(&file).cloned());
                match made {
                    Some(def) => {
                        let keep: Vec<&str> = keep.iter().map(String::as_str).collect();
                        let (copy, missing) = def.variant(name, &keep, hold);
                        if !missing.is_empty() {
                            self.say(format!("vfx: {name}: no emitter {missing:?} in {base}"));
                        }
                        self.world.register(copy);
                    }
                    None => self.say(format!("vfx: {name}: its base {base} is not in the pack")),
                }
            }
            return self.world.has(name).then(|| name.to_string());
        }
        // (ER party S5) a name the main pack lacks: the party root's (its material / mesh names get the party namespace)
        // (ER90) then the XSI root's (its own names get the xsi namespace)
        let lower = name.to_lowercase();
        // (P5) then the keyblades' root's (its own names get the kbfx namespace)
        let in_xsi = self.xsi_root.as_ref().and_then(|r| r.files.get(&lower)).cloned();
        let (file, party, xsi) = match self.files.get(&lower) {
            Some(file) => (file.clone(), false, false),
            None => match self.party_root.as_ref().and_then(|r| r.files.get(&lower)) {
                Some(file) => (file.clone(), true, false),
                None => match in_xsi {
                    Some(file) => (file, false, true),
                    None => return self.ensure_kb(&lower),
                },
            },
        };
        if self.tried.insert(file.clone()) {
            let path = match (&self.party_root, party, &self.xsi_root, xsi) {
                (Some(root), true, _, _) => root.dir.join("effects").join(format!("{file}.json")),
                // (XW-d) the XS effect's own file (effects_xwd/) when it has one
                (_, _, Some(root), true) => root.dir.join(if root.own.contains(&file.to_lowercase()) { "effects_xwd" } else { "effects" }).join(format!("{file}.json")),
                _ => self.dir.join("effects").join(format!("{file}.json")),
            };
            let mut nested = Vec::new();
            match std::fs::read_to_string(&path).map_err(|e| e.to_string()).and_then(|text| EffectDef::parse(&text)) {
                Ok(mut def) => {
                    nested = def.nested_assets();
                    if party {
                        self.party_namespace(&mut def);
                    }
                    if let (true, Some(root)) = (xsi, self.xsi_root.as_ref()) {
                        namespace_def(&mut def, XSI_NAMESPACE, &root.main_materials, &root.main_meshes);
                    }
                    self.world.register(def);
                }
                Err(e) => self.say(format!("vfx: {}: {e}", path.display())),
            }
            // The systems it spawns from inside (SQEX_SpawnParticleSystem: Thunder's bolts ...).
            for name in nested {
                if self.ensure(&name).is_none() {
                    self.say(format!("vfx: {file}: its nested system {name} is not in the pack: not drawn"));
                }
            }
            for line in self.world.take_log() {
                self.say(line);
            }
        }
        self.world.has(&file).then_some(file)
    }

    /// (ER party S5) THE PARTY'S SECOND ROOT, `PACK/party/vfx` (Donald's, Goofy's and the team attacks' effects): its
    /// effect files for the names the main pack lacks (the main pack wins a name both have), its spell actor classes
    /// (`party_host.json`) and the team link pawns' (`PACK/party/team/team_host.json`) for classes the main table lacks.
    /// Their materials and meshes that the main pack does not have are named `party:<name>` (PARTY_NAMESPACE) for the
    /// renderer. The main pack's tables, effects and names are untouched. Returns a line for the host's log.
    pub fn load_party_root(&mut self, pack_dir: &Path) -> Result<String, String> {
        let dir = pack_dir.join("party").join("vfx");
        if !dir.join("effects").is_dir() {
            return Err(format!("no party effects at {}", dir.display()));
        }
        let mut files = HashMap::new();
        for entry in std::fs::read_dir(dir.join("effects")).map_err(|e| e.to_string())?.flatten() {
            if let Some(name) = entry.file_name().to_str().and_then(|n| n.strip_suffix(".json")) {
                if !self.files.contains_key(&name.to_lowercase()) {
                    files.insert(name.to_lowercase(), name.to_string());
                }
            }
        }
        let names = |sub: &str, ext: &str| -> HashSet<String> { std::fs::read_dir(self.dir.join(sub)).map(|d| d.flatten().filter_map(|e| e.file_name().to_str().and_then(|n| n.strip_suffix(ext)).map(str::to_lowercase)).collect()).unwrap_or_default() };
        let (main_materials, main_meshes) = (names("materials", ".json"), names("meshes", ".glb"));
        let mut classes = 0;
        for file in [dir.join("party_host.json"), pack_dir.join("party").join("team").join("team_host.json")] {
            let Some(table) = std::fs::read_to_string(&file).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()) else { continue };
            for (class, roles) in table.get("projectiles").and_then(Value::as_object).into_iter().flatten() {
                if self.projectiles.contains_key(class) {
                    continue;
                }
                let roles = roles.as_object().map(|m| m.iter().map(|(role, list)| (role.clone(), list.as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect()).unwrap_or_default())).collect()).unwrap_or_default();
                self.projectiles.insert(class.clone(), roles);
                classes += 1;
            }
        }
        let line = format!("party effect root {}: {} effects, {} spell actor / team classes (their own materials and meshes as \"{PARTY_NAMESPACE}<name>\")", dir.display(), files.len(), classes);
        self.party_root = Some(PartyRoot { dir, files, main_materials, main_meshes, registered: 0 });
        Ok(line)
    }

    /// (ER party S5) Parses every effect of the party root (on a thread that may take a second). Returns how many.
    pub fn preload_party(&mut self) -> usize {
        let files: Vec<String> = self.party_root.as_ref().map(|r| r.files.values().cloned().collect()).unwrap_or_default();
        let mut n = 0;
        for file in files {
            n += usize::from(self.ensure(&file).is_some());
        }
        n
    }

    /// (ER90, XSI request 1) THE XS ABILITIES' ROOT, `PACK/xsi/vfx` (tools/sora_vfx_pack.js + sora_vfx_shaders.js `--xsi`):
    /// its effect files for the names the main pack (and the party root) lack, the main pack winning a name both have. Its
    /// materials and meshes that the main pack does not have are named `xsi:<name>` (XSI_NAMESPACE) for the renderer, which
    /// makes them from that root's own table and shaders. Read after the party root when both are wanted. Also opens the
    /// effect-group rule's name filter for the XS form effects (XSI_ALLOWED_*). The main pack's tables, effects and names
    /// are untouched; without this call (or without the root) everything is as before.
    pub fn load_xsi_root(&mut self, pack_dir: &Path) -> Result<String, String> {
        let dir = pack_dir.join("xsi").join("vfx");
        if !dir.join("effects").is_dir() {
            return Err(format!("no XS effects at {}", dir.display()));
        }
        let mut files = HashMap::new();
        let mut allowed: HashSet<String> = XSI_ALLOWED_MAIN.iter().map(|n| n.to_string()).collect();
        for entry in std::fs::read_dir(dir.join("effects")).map_err(|e| e.to_string())?.flatten() {
            if let Some(name) = entry.file_name().to_str().and_then(|n| n.strip_suffix(".json")) {
                let lower = name.to_lowercase();
                if XSI_ALLOWED_PREFIXES.iter().any(|p| lower.starts_with(p)) {
                    allowed.insert(lower.clone());
                }
                if !self.files.contains_key(&lower) && !self.party_root.as_ref().is_some_and(|r| r.files.contains_key(&lower)) {
                    files.insert(lower, name.to_string());
                }
            }
        }
        let names = |sub: &str, ext: &str| -> HashSet<String> { std::fs::read_dir(self.dir.join(sub)).map(|d| d.flatten().filter_map(|e| e.file_name().to_str().and_then(|n| n.strip_suffix(ext)).map(str::to_lowercase)).collect()).unwrap_or_default() };
        let (main_materials, main_meshes) = (names("materials", ".json"), names("meshes", ".glb"));
        // (XW-d) the effects with a file of their own in effects_xwd/ (only names of effects/), unless fx.xs_own = 0
        let own: HashSet<String> = if XS_OWN_FILES.load(std::sync::atomic::Ordering::Relaxed) {
            std::fs::read_dir(dir.join("effects_xwd")).map(|d| d.flatten().filter_map(|e| e.file_name().to_str().and_then(|n| n.strip_suffix(".json")).map(str::to_lowercase)).filter(|n| files.contains_key(n)).collect()).unwrap_or_default()
        } else {
            HashSet::new()
        };
        let mut line = format!("XS effect root {}: {} effects, {} form effects let through the name filter (their own materials and meshes as \"{XSI_NAMESPACE}<name>\")", dir.display(), files.len(), allowed.len());
        if !own.is_empty() {
            line += &format!("; (XW-d) {} of them read from effects_xwd/ (their own materials, fragment pieces; fx.xs_own = 0 switches it off)", own.len());
        }
        self.xsi_root = Some(XsiRoot { dir, files, main_materials, main_meshes, allowed, own });
        Ok(line)
    }

    /// (ER90) Parses every effect of the XSI root (on a thread that may take a second). Returns how many.
    pub fn preload_xsi(&mut self) -> usize {
        let files: Vec<String> = self.xsi_root.as_ref().map(|r| r.files.values().cloned().collect()).unwrap_or_default();
        let mut n = 0;
        for file in files {
            n += usize::from(self.ensure(&file).is_some());
        }
        n
    }

    /// (ER90) The XSI root is read.
    pub fn has_xsi_root(&self) -> bool {
        self.xsi_root.is_some()
    }

    /// (P5 WEAPON DRAW) THE KEYBLADES' ROOT, `PACK/keyblades/vfx` (tools/sora_keyblades_pack.js, its own shading table and
    /// shaders: kh-er-render vfx.rs `kbfx:`): its effect files for the names the other roots lack (read after the party and
    /// XSI roots). Its materials and meshes the main pack lacks are named `kbfx:<name>`. Without this call (or the root)
    /// everything is as before. Returns a line for the host's log.
    pub fn load_kb_root(&mut self, pack_dir: &Path) -> Result<String, String> {
        let dir = pack_dir.join("keyblades").join("vfx");
        if !dir.join("effects").is_dir() {
            return Err(format!("no keyblade effects at {}", dir.display()));
        }
        let mut files = HashMap::new();
        for entry in std::fs::read_dir(dir.join("effects")).map_err(|e| e.to_string())?.flatten() {
            if let Some(name) = entry.file_name().to_str().and_then(|n| n.strip_suffix(".json")) {
                let lower = name.to_lowercase();
                let elsewhere = self.files.contains_key(&lower) || self.party_root.as_ref().is_some_and(|r| r.files.contains_key(&lower)) || self.xsi_root.as_ref().is_some_and(|r| r.files.contains_key(&lower));
                if !elsewhere {
                    files.insert(lower, name.to_string());
                }
            }
        }
        let names = |sub: &str, ext: &str| -> HashSet<String> { std::fs::read_dir(self.dir.join(sub)).map(|d| d.flatten().filter_map(|e| e.file_name().to_str().and_then(|n| n.strip_suffix(ext)).map(str::to_lowercase)).collect()).unwrap_or_default() };
        let (main_materials, main_meshes) = (names("materials", ".json"), names("meshes", ".glb"));
        let line = format!("keyblade effect root {}: {} effects (their own materials and meshes as \"{KB_NAMESPACE}<name>\")", dir.display(), files.len());
        self.kb_root = Some(KbRoot { dir, files, main_materials, main_meshes });
        Ok(line)
    }

    /// (LOADIN) The keyblades' root's effect files whose (lower-case) names start with `prefix` (e.g. `vw_so030_`: a keyblade's
    /// own trails, appear / disappear and hit effects), sorted; empty without the root.
    pub fn kb_files_with_prefix(&self, prefix: &str) -> Vec<String> {
        let prefix = prefix.to_lowercase();
        let mut out: Vec<String> = self.kb_root.as_ref().map(|r| r.files.keys().filter(|k| k.starts_with(&prefix)).cloned().collect()).unwrap_or_default();
        out.sort();
        out
    }

    /// (LOADIN) THE SLOTTED KEYBLADES' OWN EFFECTS queued to be parsed ahead of their first spawn (`preload_step`): the hand
    /// keyblade's `trail`, the names their data spawns (`spawns`: kh-core `keyblade::effect_names`) and every effect of the keyblades'
    /// root of their base models (`bases`, e.g. `w_so030`: its `vw_so030_*` trails, appear / disappear and hit effects). Their form
    /// weapons' effects are not among them (they come with the form, as before). Returns how many were queued.
    pub fn queue_keyblade_effects(&mut self, trail: Option<&str>, spawns: &[String], bases: &[String]) -> usize {
        let mut names: Vec<String> = trail.iter().map(|t| t.to_string()).chain(spawns.iter().cloned()).collect();
        for base in bases {
            names.extend(self.kb_files_with_prefix(&format!("v{base}_")));
        }
        self.queue_preload(&names)
    }

    /// (LOADIN) Names these effects to be parsed ahead of their first spawn (`preload_step`, on the host's own thread, a few at a
    /// time); a name queued before, or already parsed, is skipped. Returns how many were queued.
    pub fn queue_preload(&mut self, names: &[String]) -> usize {
        let mut queued = 0;
        for name in names {
            let lower = name.to_lowercase();
            let parsed = self.tried.contains(name.as_str()) || self.files.get(&lower).is_some_and(|f| self.tried.contains(f)) || self.kb_root.as_ref().and_then(|r| r.files.get(&lower)).is_some_and(|f| self.tried.contains(f));
            if self.preload_seen.insert(lower) && !parsed {
                self.preload_queue.push_back(name.clone());
                queued += 1;
            }
        }
        queued
    }

    /// (LOADIN) Parses queued effects (`queue_preload`) until `budget` is spent (at least one). Some(how many of this call's were
    /// found) when the queue became empty in this call: their materials and meshes are then among `assets`, for the renderer to
    /// make ready before their first spawn. None: nothing was queued, or more remain.
    pub fn preload_step(&mut self, budget: std::time::Duration) -> Option<usize> {
        if self.preload_queue.is_empty() {
            return None;
        }
        let started = std::time::Instant::now();
        let mut known = 0;
        while let Some(name) = self.preload_queue.pop_front() {
            known += usize::from(self.ensure(&name).is_some());
            if started.elapsed() >= budget {
                break;
            }
        }
        self.preload_queue.is_empty().then_some(known)
    }

    /// (P5) The keyblades' root is read.
    pub fn has_kb_root(&self) -> bool {
        self.kb_root.is_some()
    }

    /// (P5) An effect of the keyblades' root (its name lower case), registered on first use (with its nested systems).
    fn ensure_kb(&mut self, lower: &str) -> Option<String> {
        let root = self.kb_root.as_ref()?;
        let file = root.files.get(lower)?.clone();
        if self.tried.insert(file.clone()) {
            let path = root.dir.join("effects").join(format!("{file}.json"));
            let mut nested = Vec::new();
            match std::fs::read_to_string(&path).map_err(|e| e.to_string()).and_then(|text| EffectDef::parse(&text)) {
                Ok(mut def) => {
                    nested = def.nested_assets();
                    namespace_def(&mut def, KB_NAMESPACE, &root.main_materials, &root.main_meshes);
                    self.world.register(def);
                }
                Err(e) => self.say(format!("vfx: {}: {e}", path.display())),
            }
            for name in nested {
                if self.ensure(&name).is_none() {
                    self.say(format!("vfx: {file}: its nested system {name} is not in the pack: not drawn"));
                }
            }
            for line in self.world.take_log() {
                self.say(line);
            }
        }
        self.world.has(&file).then_some(file)
    }

    /// (P5 WEAPON DRAW) THE KEYBLADE IN HIS HAND (kh-er sora/kbdraw.rs on a change): its trail (`kb.<KEY>.fx.trail`'s asset
    /// name; None = the Kingdom Key's VW_SO010_LOC0), its skeleton's sockets (`<model>/sockets.json` `weapon_sockets`, the
    /// format of host.json's: vfx_top / vfx_bottom at its own blade length) and the name prefixes of its own effects (its
    /// models': `vw_so080` ...) that the other-keyblade rule lets through. All None / empty: the Kingdom Key as before.
    pub fn set_keyblade(&mut self, trail: Option<&str>, sockets: Option<&Value>, allowed: &[String]) {
        if trail.is_none() && sockets.is_none() && allowed.is_empty() {
            self.kb = None;
            return;
        }
        let sockets = sockets.and_then(Value::as_object).map(|m| m.iter().map(|(name, s)| (name.clone(), Socket { bone: s["bone"].as_str().unwrap_or("").to_string(), location: vec3_of(s.get("local_location"), Vec3::ZERO), rotation: vec3_of(s.get("local_rotation"), Vec3::ZERO) })).collect()).unwrap_or_default();
        self.kb = Some(KbFx { trail: trail.map(str::to_string), sockets, allowed: allowed.iter().map(|a| a.to_lowercase()).collect() });
    }

    /// (ER90) An XS form effect the name filter lets through (only with the XSI root read).
    fn xsi_allowed(&self, lower: &str) -> bool {
        self.xsi_root.as_ref().is_some_and(|r| r.allowed.contains(lower) || lower.starts_with("vw_so191_") || lower.starts_with("vw_so192_"))
    }

    /// (ER party S5) Whether an effect of that name is in the pack or the party root (registered on the way).
    pub fn has_effect(&mut self, name: &str) -> bool {
        self.ensure(name).is_some()
    }

    /// (ER party S5) The party root is read.
    pub fn has_party_root(&self) -> bool {
        self.party_root.is_some()
    }

    /// (ER party S5) The party root's own material / mesh names get the party namespace.
    fn party_namespace(&mut self, def: &mut EffectDef) {
        let Some(root) = self.party_root.as_mut() else { return };
        root.registered += 1;
        let material = |name: &mut String| {
            if !name.is_empty() && !name.starts_with(PARTY_NAMESPACE) && !root.main_materials.contains(&name.to_lowercase()) {
                *name = format!("{PARTY_NAMESPACE}{name}");
            }
        };
        let mesh = |name: &mut String| {
            if !name.is_empty() && !name.starts_with(PARTY_NAMESPACE) && !root.main_meshes.contains(&name.to_lowercase()) {
                *name = format!("{PARTY_NAMESPACE}{name}");
            }
        };
        for e in &mut def.emitters {
            material(&mut e.material);
            if let Some(d) = e.decal.as_mut() {
                material(&mut d.material);
            }
            if let Some(m) = e.mesh.as_mut() {
                mesh(m);
            }
            if let Some(s) = e.sub_meshes.as_mut() {
                s.list.iter_mut().for_each(mesh);
            }
        }
    }

    /// (ER party S5) THE PARTY OF THIS STEP, before `step`: every member present (`kh_core::PartyMember.0` -> his pose)
    /// and the party's spell actors / team props (body-effect handle, position, flight direction). An empty list = no
    /// party (their effects keep their last places until they end).
    pub fn set_party(&mut self, members: Vec<(u8, PartyMemberFx)>, projectiles: Vec<(u32, Vec3, Vec3)>) {
        self.party_members = members.into_iter().collect();
        self.party_projectiles = projectiles;
    }

    /// (FREEZE-FX) The targets' bones as of this step (replaces the last set; world place and turn, cm, core axes): the
    /// effects anchored on a target whose particles start on its mesh (`SQEX_VertexSpawn` bUseAttachMeshVertex: KH3's frozen
    /// look) spawn on its own bones, fitted to its body.
    pub fn set_target_bones(&mut self, bones: Vec<(TargetId, BoneMap)>) {
        self.target_bones = bones.into_iter().map(|(t, b)| (t.0, std::sync::Arc::new(b))).collect();
    }

    /// (ER party S5) THE LOOPS STILL RUNNING per owner (`kh_core::PartyMember.0`, 255 = the team pawns): the party's
    /// persistent effects the core has not stopped, and its endless one-shots not yet ended. After a move this is 0.
    pub fn party_loops(&self) -> Vec<(u8, usize)> {
        let mut out: Vec<(u8, usize)> = Vec::new();
        for live in self.live.iter().filter(|l| kh_core::host::party::is_party_handle(l.handle.0) && !l.stopped && (l.persistent || l.ends_with_animation) && self.world.is_alive(l.id)) {
            let owner = kh_core::host::party::handle_member(live.handle.0).map_or(255, |m| m.0);
            match out.iter_mut().find(|(o, _)| *o == owner) {
                Some(e) => e.1 += 1,
                None => out.push((owner, 1)),
            }
        }
        out.sort();
        out
    }

    /// (ER party S5) Everything the party started ends (the party is switched off or hidden: no loop outlives it).
    /// Returns how many effects were stopped.
    pub fn stop_party(&mut self, immediate: bool) -> usize {
        let mut n = 0;
        // (MIMIC-FX) a copy's (kh_audio::copies: the Mimic Tears' copies, never the party's) ends with the copy (`stop_member`)
        let copy = |h: u32| kh_core::host::party::handle_member(h).is_some_and(|m| kh_audio::copies::copy_of(m.0).is_some());
        self.kept_drives.retain(|k| !kh_core::host::party::is_party_handle(k.0 .0) || copy(k.0 .0)); // (FREEZE-FX)
        for live in self.live.iter_mut().filter(|l| kh_core::host::party::is_party_handle(l.handle.0) && !l.stopped && !copy(l.handle.0)) {
            live.stopped = true;
            self.world.stop(live.id, immediate);
            n += 1;
        }
        n
    }

    /// (FX5) Everything SORA started ends (he left the party: a host does not step him, so his core never carries out a
    /// stop; nothing of an absent Sora may be drawn). His effects are those of his own handles (below the party's ranges,
    /// `kh_core::host::party::PARTY_HANDLE_BASE`); the party's effects and the free ones (`free_spawn`: save points,
    /// prizes) are not touched. Returns how many effects were stopped.
    pub fn stop_sora(&mut self, immediate: bool) -> usize {
        let mut n = 0;
        self.kept_drives.retain(|k| k.0 .0 >= kh_core::host::party::PARTY_HANDLE_BASE); // (FREEZE-FX)
        for live in self.live.iter_mut().filter(|l| l.handle.0 < kh_core::host::party::PARTY_HANDLE_BASE && (immediate || !l.stopped)) { // (immediate: the tails still fading out too)
            live.stopped = true;
            self.world.stop(live.id, immediate);
            n += 1;
        }
        n
    }

    /// A bone's or socket's world transform this step (KH3 space), from the pose.
    fn socket_world(&self, s: &Scene, name: Option<&str>) -> Xform {
        let out = s.out;
        let component = Xform::from_rotator(out.position, 0.0, out.yaw - 90.0, 0.0, 1.0);
        let Some(name) = name else { return component };
        // (AUDIT-A SHIELD) a shield finisher pawn's notify socket (`pawn:<row>`: Pegasus / the Chariot / the Frying Pan stand at
        // his root with his yaw, their clip in step with his heaviest sample): kh-core shield::pawn_socket, his ACTOR frame
        if name.starts_with("pawn:") {
            let time = out.anim_ticked.body_samples()[0].2;
            if let Some((p, r, absolute)) = kh_core::combat::forms::shield::pawn_socket(name, time) {
                let actor = Xform::from_rotator(out.position, 0.0, out.yaw, 0.0, 1.0);
                let local = Xform { position: p, rotation: kh_vfx::rotator(r.x, r.y, r.z), scale: Vec3::ONE };
                // (AUDIT-A PLACEMENT) the thrown shield's / pan's own M_B414 effects: on the weapon where it flies
                let mut x = self.thrown_anchor(s, name, &local).unwrap_or_else(|| actor.then(&local));
                if absolute {
                    x.rotation = kh_vfx::rotator(r.x, r.y, r.z);
                }
                return x;
            }
        }
        // (AUDIT-B F-SHIP) a form weapon's / pawn's own bone (`kb:<model>:<mount>:<bone>`): its mount (one of his sockets, or
        // his component for `root`) x the weapon's component pose of the sequence he plays now x the bone / socket
        if let Some(k) = kbweapon::parse(name) {
            // (AUDIT-A PLACEMENT) a part KH3 threw (placement::ThrownWeapon: the flag's M_F413 effects): on it where it flies,
            // under its own throw clip
            if let Some(t) = s.sora.combat().thrown.iter().find(|t| t.model == k.model) {
                let weapon = Xform { position: t.position, rotation: Mat3::from_quat(t.rotation), scale: Vec3::ONE };
                return self.kb_rigs.local(k.model, k.bone, t.clip().1, t.clip_time()).map_or(weapon, |local| weapon.then(&local));
            }
            // (AUDIT-A PLACEMENT) a weapon clip's ChangeAttachParent window: the part (and so its own effects) on that bone
            // (AUDIT-B W-HANDOVER) or the weapon clip's own window whoever equips it (placement::clip_parent: a shotlock's equip set)
            let (rp_seq, rp_time) = (s.out.anim_ticked.body_samples()[0].0, s.out.anim_ticked.time);
            let reparent = kh_core::combat::forms::placement::part_parent(s.sora.combat().form.cmd, k.model, rp_seq, rp_time).or_else(|| kh_core::combat::forms::placement::clip_parent(k.model, rp_seq, rp_time)).filter(|_| !matches!(k.mount, "root" | "tower" | "accompany"));
            // (FORMWEP-FIX) a transform unit the equipment attaches with no socket stands on his mesh component while it plays
            // its own clip (whatever mount the row names: the sheets' and weaponattach's `R_wep_skt`), as the renderer draws it
            let (sequence, time) = (s.out.anim_ticked.body_samples()[0].0, s.out.anim_ticked.time);
            let logical = kh_core::combat::forms::weapon_logical(s.sora.combat().form.cmd, s.out.anim_ticked.name);
            let on_mesh = reparent.is_none() && weaponattach::on_mesh_now(s.sora, k.model) && self.kb_rigs.plays_own(k.model, sequence, logical);
            let mount = if let Some(bone) = reparent { self.socket_world(s, Some(bone)) } else if k.mount == "root" || on_mesh { component } else if k.mount == "tower" { tower_component(s).unwrap_or(component) /* (AUDIT-A STAFF) */ } else if k.mount == "accompany" { s.sora.finish().and_then(|f| f.accompany).map_or(component, |(at, yaw)| Xform::from_rotator(at, 0.0, yaw - 90.0, 0.0, 1.0)) /* (AUDIT-B F-HAMMER) the finish's accompany pawn where it stands (Clock Drill's Clock) */ } else { self.socket_world(s, Some(k.mount)) };
            // (AUDIT-B W-DATA) a blend-space play of the form's set: the weapon's row of its logical name (Twin Yo-Yos' MagicFire4)
            return self.kb_rigs.local_logical(k.model, k.bone, sequence, logical, time).map_or(mount, |local| mount.then(&local));
        }
        let bone = |bone: &str| s.tap.bone_world(bone).map(|t| Xform { position: t.translation, rotation: Mat3::from_quat(t.rotation), scale: t.scale });
        let relative = |s: &Socket| Xform { position: s.location, rotation: kh_vfx::rotator(s.rotation.x, s.rotation.y, s.rotation.z), scale: Vec3::ONE };
        // (FINISH-AUDIT2) `wep:<socket>`: the Keyblade in his hand as the attach OBJECT (weaponattach.rs): its own socket (on
        // its bone `wep`), else (a name its mesh lacks, e.g. Sonic Rave's `vfx_root`: no Keyblade skeleton but w_so081 / 111 /
        // 114 has one, Character/wep/*/mdl/*_Skeleton) its mesh COMPONENT, as UE attaches to a missing socket: the component
        // stands on R_wep_skt with no turn of its own, so it has neither the bone `wep`'s turn nor a socket's (a camera-facing
        // mesh with roll takes its roll from this frame: with the bone's 180 degrees the thrust ran back from the grip)
        if let Some(inner) = name.strip_prefix("wep:") {
            let Some(hand) = self.sockets.get("R_wep_skt") else { return component };
            let mesh = bone(&hand.bone).map_or(component, |b| b.then(&relative(hand)));
            let wep = Xform { position: Vec3::ZERO, rotation: kh_vfx::rotator(0.0, 0.0, 180.0), scale: Vec3::ONE };
            let own = |n: &str| self.kb.as_ref().and_then(|k| k.sockets.get(n)).or_else(|| self.weapon_sockets.get(n));
            return match own(inner) {
                Some(socket) => mesh.then(&wep).then(&relative(socket)),
                // (FINISH-AUDIT2, THE USER 2026-11-07: "the misplaced effects should be coming from the keyblade tip") a name
                // the Keyblade's mesh lacks: at its tip (`vfx_top`) in the component's turn, never its grip
                None => own("vfx_top").map_or(mesh, |tip| Xform { position: mesh.then(&wep).then(&relative(tip)).position, ..mesh }),
            };
        }
        if let Some(socket) = self.sockets.get(name) {
            return bone(&socket.bone).map_or(component, |b| b.then(&relative(socket)));
        }
        // (P5) the keyblade in his hand: its own sockets first (its blade's length), the Kingdom Key's for a name it lacks
        if let Some(socket) = self.kb.as_ref().and_then(|k| k.sockets.get(name)).or_else(|| self.weapon_sockets.get(name)) {
            // The weapon root sits on R_wep_skt; bone `wep` is turned 180 degrees about X in the
            // weapon's component space (kh_sk and wep each carry a quarter turn; weapon manifest).
            let Some(hand) = self.sockets.get("R_wep_skt") else { return component };
            let root = bone(&hand.bone).map_or(component, |b| b.then(&relative(hand)));
            let wep = Xform { position: Vec3::ZERO, rotation: kh_vfx::rotator(0.0, 0.0, 180.0), scale: Vec3::ONE };
            return root.then(&wep).then(&relative(socket));
        }
        // A stand-in without a pose: one metre above his origin, as the markers do.
        bone(name).unwrap_or(Xform { position: out.position + Vec3::Z * if s.tap.has_pose() { 0.0 } else { 100.0 }, ..component })
    }

    /// (AUDIT-A PLACEMENT) A `pawn:` anchor of a THROWN weapon (kh-core shield::thrown_row) while KH3's weapon flies
    /// (kh-core placement::ThrownWeapon): the row's transform `local` is baked in Sora's actor frame at the throw (his socket
    /// there x the weapon's bone at its clip's time 0 x the notify's offset); undone and put on the flying weapon under its
    /// own throw clip now: weapon x bone(t) x bone(0)^-1 x socket(throw)^-1 x local. None: not thrown / not flying.
    fn thrown_anchor(&self, s: &Scene, name: &str, local: &Xform) -> Option<Xform> {
        let (model, socket) = kh_core::combat::forms::shield::thrown_row(name)?;
        let t = s.sora.combat().thrown.iter().find(|t| t.model == model)?;
        let bone = if socket == "None" { "" } else { socket };
        let (_, sequence) = t.clip();
        let (at0, now) = (self.kb_rigs.local(model, bone, sequence, 0.0)?, self.kb_rigs.local(model, bone, sequence, t.clip_time())?);
        // (the weapon's bones carry its import scale: undone too; `then` scales the child's place by the parent's scale)
        let inverse = |x: &Xform| {
            let (r, scale) = (x.rotation.transpose(), x.scale.recip());
            Xform { position: -(scale * (r * x.position)), rotation: r, scale }
        };
        let (hp, hq) = t.socket_at_throw();
        let hand = Xform { position: hp, rotation: Mat3::from_quat(hq), scale: Vec3::ONE };
        let weapon = Xform { position: t.position, rotation: Mat3::from_quat(t.rotation), scale: Vec3::ONE };
        Some(weapon.then(&now).then(&inverse(&at0)).then(&inverse(&hand)).then(local))
    }

    /// (AUDIT-B W-VFX) The beam ends a live effect's TypeDataSQEX_Thunder emitters ask for (kh-vfx `World::beam_asks`), on
    /// the mesh it hangs on (KH3 0x144250690: GetSocketTransform of the component's attach parent, world location):
    /// - one beam from the anchor's own socket to its EndSocketName (`kb:...:<socket>:to<end>`, SetThunderAttachBones);
    /// - with a bone list, beam i from bones[i] to bones[i + 1] of the same mesh.
    ///
    /// Only a `kb:` anchor names that mesh (a form weapon / pawn: every pack row of a thunder effect is one); an unknown
    /// bone is the mesh's component (UE's GetSocketTransform), as `socket_world` resolves it.
    fn beam_ends(&self, live: &Live, id: kh_vfx::EffectId, s: &Scene) -> Vec<(usize, Vec<(Vec3, Vec3)>)> {
        let asks = self.world.beam_asks(id);
        if asks.is_empty() {
            return Vec::new();
        }
        let Anchor::Sora(Some(socket), _) = &live.anchor else { return Vec::new() };
        let Some(k) = kbweapon::parse(socket) else { return Vec::new() };
        let at = |bone: &str| kbweapon::with_bone(socket, bone).map(|name| self.socket_world(s, Some(&name)).position);
        asks.iter()
            .map(|ask| {
                let ends = match &ask.bones {
                    Some(bones) => bones.windows(2).filter_map(|w| Some((at(&w[0])?, at(&w[1])?))).collect(),
                    None => k.to.and_then(|to| Some((at(k.bone)?, at(to)?))).into_iter().collect(),
                };
                (ask.emitter, ends)
            })
            .collect()
    }

    fn anchor_xform(&self, live: &Live, s: &Scene) -> Option<Xform> {
        let place = |base: Xform| {
            let p = &live.placement;
            let mut x = base.then(&Xform { position: p.offset, rotation: kh_vfx::rotator(p.rotation.x, p.rotation.y, p.rotation.z), scale: p.scale });
            if p.absolute_rotation {
                x.rotation = kh_vfx::rotator(p.rotation.x, p.rotation.y, p.rotation.z);
            }
            x
        };
        Some(match &live.anchor {
            // (DON-KIT) a party actor's body spawned by its effect name (not a projectile class: Donald's element missiles,
            // kh-party pad/donald_shots.rs) follows the actor while the party reports it (kh-app vfx.rs, the same rule)
            Anchor::World(x) if live.persistent && self.party_projectiles.iter().any(|p| p.0 == live.handle.0) => {
                let p = self.party_projectiles.iter().find(|p| p.0 == live.handle.0)?;
                let forward = p.2.normalize_or_zero();
                let rotation = if forward == Vec3::ZERO { Mat3::IDENTITY } else { kh_vfx::rotator(forward.z.clamp(-1.0, 1.0).asin().to_degrees(), forward.y.atan2(forward.x).to_degrees(), 0.0) };
                Xform { position: p.1, rotation, scale: x.scale }
            }
            // (SHOT-C avatar, INT-F) Ghost Horde / Phantom Rush's avatars: each one's persistent particle (spawned at his lock
            // point) moves with its avatar every frame (`PawnMagic::shot_avatars`: position, rotation (pitch, yaw, roll))
            // (MIMIC-FX) a Sora instance's core knows its own effects by its own number (`core_handle`)
            Anchor::World(x) if live.persistent && s.sora.magic().shot_avatars.iter().any(|a| a.effect == Some(live.core_handle())) => {
                let a = s.sora.magic().shot_avatars.iter().find(|a| a.effect == Some(live.core_handle()))?;
                Xform { position: a.position, rotation: kh_vfx::rotator(a.rotation.x, a.rotation.y, a.rotation.z), scale: x.scale }
            }
            Anchor::World(x) => *x,
            Anchor::Sora(socket, offset) => {
                let mut x = self.socket_world(s, socket.as_deref());
                let (sin, cos) = s.out.yaw.to_radians().sin_cos();
                x.position += Vec3::new(offset.x * cos - offset.y * sin, offset.x * sin + offset.y * cos, offset.z);
                place(x)
            }
            Anchor::Target(target, offset) => place(Xform::at((s.target)(*target)? + *offset)),
            // (ER party S5) on the member's latest pose (kh-app vfx.rs Anchor::Party, the same rule)
            Anchor::Party(member, bone, offset) => {
                let (position, rotation) = self.party_members.get(member)?.xform(bone.as_deref(), *offset);
                place(Xform { position, rotation: Mat3::from_quat(rotation), scale: Vec3::ONE })
            }
            Anchor::Projectile => {
                // (ER party S5) a party spell actor / team prop, else one of Sora's
                // (MIMIC-FX) a Sora instance's actors are in HIS core (the scene's), known by his core's own number
                let own = live.core_handle();
                let (position, forward) = match self.party_projectiles.iter().find(|p| p.0 == live.handle.0) {
                    Some(p) => (p.1, p.2),
                    // (F-ARROWGUN) ... else a gun form's bullet
                    None => s.sora.magic().projectile_views().find(|p| p.effect == Some(own)).map(|p| (p.position, p.forward)).or_else(|| s.sora.combat().arrowgun.views().find(|b| b.effect == Some(own)).map(|b| (b.position, b.forward)))
                        // (F-STAFF) ... else a Mirage Staff beam or mirror image
                        .or_else(|| s.sora.combat().staff.beams.views().find(|b| b.effect == Some(own)).map(|b| (b.position, b.forward)))
                        .or_else(|| s.sora.combat().staff.avatars.views().find(|a| a.effect == Some(own)).map(|a| (a.position, Vec3::new(a.yaw.to_radians().cos(), a.yaw.to_radians().sin(), 0.0))))?,
                };
                // The actor's X axis is its flight direction.
                let forward = forward.normalize_or_zero();
                let rotation = if forward == Vec3::ZERO { Mat3::IDENTITY } else { kh_vfx::rotator(forward.z.clamp(-1.0, 1.0).asin().to_degrees(), forward.y.atan2(forward.x).to_degrees(), 0.0) };
                Xform { position, rotation, scale: Vec3::ONE }
            }
        })
    }

    /// What a spawn of the core means: the effects to start, each with its anchor.
    fn resolve_spawn(&mut self, spawn: &EffectSpawn) -> Vec<(String, Anchor, bool)> {
        let name = kh_vfx::base_name(spawn.name);
        let mut out: Vec<(String, Anchor, bool)> = Vec::new();
        // The shared clips also carry the rows of the other keyblades' forms (vw_so172_*,
        // vw_so200_* ...), which the Kingdom Key never spawns (effect-group rule, spec 4.2 / 7.4).
        let lower = name.to_lowercase();
        // (ER90) with the XSI root read: the XS moves' form effects pass (kh-app vfx.rs, the same rule + Spectral Rays)
        // (P5) the keyblade in his hand: its own effects (its models' names) pass
        // (F-LIGHTDARK) Double Form's effects are vw_so220_* (w_so200_Wep / w_so210_Wep name them in their own hit effects): they
        // pass with Oathkeeper or Oblivion (models w_so200 / w_so210).
        let double_form = lower.starts_with("vw_so220") && self.kb.as_ref().is_some_and(|k| k.allowed.iter().any(|a| a == "vw_so200" || a == "vw_so210"));
        // (KBFX) the game also names a few groups with four digits (Hero's Origin's level-ups vw_so0033_lv1 / vw_so0034_lv2):
        // they belong to the three-digit model (vw_so033 / vw_so034)
        let lower = four_digit_group(&lower).unwrap_or(lower);
        if lower.starts_with("vw_so") && !double_form && !lower.starts_with("vw_so010") && !lower.starts_with("vw_sor_") && !self.xsi_allowed(&lower) && !self.kb.as_ref().is_some_and(|k| k.allowed.iter().any(|a| lower.starts_with(a.as_str()))) {
            self.say(format!("vfx: \"{name}\" belongs to another keyblade (effect-group rule, spec 7.4): not drawn"));
            return out;
        }
        let rotated = |p: Vec3| Xform::from_rotator(p, spawn.rotation.x, spawn.rotation.y, spawn.rotation.z, spawn.scale);
        let anchor = match spawn.at {
            Attach::World(p) => Anchor::World(rotated(p)),
            Attach::Sora { bone, offset } => Anchor::Sora(bone.map(str::to_string), offset),
            Attach::Target { target, offset } => Anchor::Target(target, offset),
            // (party stream E) This host has no party members: such an effect stays at its offset.
            // (ER party S5) it has: on the member's pose (`set_party`)
            Attach::Party { member, bone, offset } => Anchor::Party(member.0, bone.map(str::to_string), offset),
        };
        // (ER party S5) Sora's own spawn rules (his Keyblade's cast socket, his heal) never take a party spawn
        let party = matches!(anchor, Anchor::Party(..));
        if name == "key_trail0" && !party {
            // (P5) the trail of the keyblade in his hand (`kb.<KEY>.fx.trail`), the Kingdom Key's without one
            let trail = self.kb.as_ref().and_then(|k| k.trail.clone()).filter(|t| self.ensure(t).is_some()).unwrap_or_else(|| "VW_SO010_LOC0".to_string());
            out.push((trail, Anchor::Sora(Some("vfx_top".to_string()), Vec3::ZERO), true));
        } else if let Some(roles) = self.projectiles.get(spawn.name).cloned() {
            // A spell actor: its ParticleComp template follows it (the persistent event); its
            // MyFireEffects play at the fire position with the actor's rotation (the core's second
            // event, id FIRE_EFFECTS_ID: the muzzle for a cast, the actor's own place for a successor).
            if spawn.id.0 == kh_core::magic::projectile::FIRE_EFFECTS_ID {
                for effect in roles.get("fire").into_iter().flatten() {
                    out.push((effect.clone(), anchor.clone(), false));
                }
            } else {
                for effect in roles.get("body").into_iter().flatten() {
                    out.push((effect.clone(), Anchor::Projectile, false));
                }
            }
        } else if spawn.persistent && kh_core::combat::forms::arrowgun::is_bullet_particle(spawn.name) {
            // (F-ARROWGUN) a gun form's bullet: its ParticleComp template follows it (the core's BulletView)
            out.push((name.to_string(), Anchor::Projectile, false));
        } else if spawn.persistent && kh_core::combat::forms::staff::is_projectile_particle(spawn.name) {
            // (F-STAFF) a Mirage Staff beam / mirror image: its ParticleComp template follows it (BeamView / AvatarView)
            out.push((name.to_string(), Anchor::Projectile, false));
        } else if name == "sora_kaihuku" && !party {
            // The heal on his body: the Cure rows of ead_com_abn0 by the group of the cast
            // (1420 Cure, 1422 Cura, 1424 Curaga; the item's own group is not resolved: Cure's).
            let (tier, socket) = match self.cure_group {
                1424 => (2, "vfx_cure"),
                1422 => (1, "vfx_abn_center"),
                _ => (0, "vfx_abn_center"),
            };
            for part in ["hed", "bdy"] {
                out.push((format!("vm_cur_{part}{tier}"), Anchor::Sora(Some(socket.to_string()), Vec3::ZERO), false));
            }
        } else if name == "DOWNIMPACT" {
            out.push(("vp_hit_zmn0".to_string(), anchor, false));
        } else if name == "clash" {
            // The weapon clash: the game mode's reflect effect (TresGameModeLoad GuardReflectEffect).
            out.push(("vs_com_haj00".to_string(), anchor, false));
        } else if name.ends_with("_cal0") && !party {
            // The cast effect sits on the Keyblade's socket vfx_call (ead_wep_cal0), not its root.
            if let Some(group) = Some(spawn.id.0).filter(|g| (1420..=1425).contains(g)) {
                self.cure_group = group;
            }
            out.push((name.to_string(), Anchor::Sora(Some("vfx_call".to_string()), Vec3::ZERO), false));
        } else {
            // A spawn "on Sora" that names no socket sits where the effect's attach row puts it.
            let anchor = match (&anchor, self.default_sockets.get(&name.to_lowercase())) {
                (Anchor::Sora(None, offset), Some(socket)) => Anchor::Sora(Some(socket.clone()), *offset),
                _ => anchor,
            };
            // The Cure rows come in pairs (head + body); the core names the head.
            if let Some(tier) = name.strip_prefix("vm_cur_hed") {
                out.push((format!("vm_cur_bdy{tier}"), anchor.clone(), false));
            }
            // (VFX-2) an attach-data asset played with a group (SQEX_StructParticleAttachData rows per group, packed by
            // tools/vfx_attach_effects.js as `<asset>__g<group>`): that group's rows before the asset's group-0 file.
            // Snakebite's charge ead_shf_tame0 (group 0x72B = 1835) has no group-0 rows at all.
            let grouped = format!("{name}__g{}", spawn.id.0);
            out.push((if self.ensure(&grouped).is_some() { grouped } else { name.to_string() }, anchor, false));
        }
        out
    }

    fn start(&mut self, handle: EffectHandle, spawn: &EffectSpawn, s: &Scene) {
        // A rail node of the Blizzard bolt: no particle system. The trail piece from the node
        // before it (the same bolt's: laid within half a second and four metres) is drawn from
        // the ice trail's meshes.
        if spawn.name == "TresRailSlideActor" {
            if let (Attach::World(point), Some(rail)) = (spawn.at, self.rail.as_ref()) {
                let now = self.seconds;
                if let Some((previous, at)) = self.rail_last {
                    let length = previous.distance(point);
                    if now - at < 0.5 && length > 10.0 && length < 400.0 {
                        let k = self.rail_pieces.len() + (now * 60.0) as usize;
                        let r = |salt: usize| ((k * 7919 + salt * 104_729) % 1000) as f32 / 999.0;
                        let scale = rail.scale + rail.scale_random.0 + (rail.scale_random.1 - rail.scale_random.0) * Vec2::new(r(1), r(2));
                        self.rail_pieces.push(RailPiece { from: previous, to: point, segment: k % rail.segments.len(), scale, born: now });
                    }
                }
                self.rail_last = Some((point, now));
                self.spawned += 1;
            }
            return;
        }
        // (AUDIT-B F-SHIP) a FORM WEAPON'S trail: `ead_key_trail0` (the weapon's DesignatedAttachData, whose rows ask for its
        // common `key_trail0`) on the socket a form unit stands on runs between THAT weapon's vfx_top / vfx_bottom
        let form_trail = form_trail_anchor(spawn, s);
        // (AUDIT-C TF) + Double Form's attach data asset (AirDodge N054's vw_so220_warp)
        let resolved = match (&form_trail, attach_data_rows(spawn, s).or_else(|| tf_attach_rows(spawn))) {
            (Some(_), _) => self.resolve_spawn(&EffectSpawn { name: "key_trail0", ..*spawn }),
            // (AUDIT-C SW) an attach data asset of a form's clips: its rows, each where its own row attaches it
            (None, Some(rows)) => rows,
            (None, None) => self.resolve_spawn(spawn),
        };
        for (effect, anchor, trail) in resolved {
            let mut anchor = match (&form_trail, trail) {
                (Some(a), true) => Anchor::Sora(Some(a.clone()), Vec3::ZERO),
                _ => anchor,
            };
            let Some(file) = self.ensure(&effect) else {
                self.say(format!("vfx: \"{effect}\" (spawned as \"{}\") is no effect of the pack: nothing is drawn for it", spawn.name));
                continue;
            };
            let socket = match &anchor {
                Anchor::Sora(Some(socket), _) => socket.clone(),
                _ => String::new(),
            };
            // ((IMPL_ER53) a host's copy `BASE#...` is placed as its base)
            let placed_as = file.split('#').next().unwrap_or(&file).to_lowercase();
            let mut placement = self.placements.get(&(placed_as, socket.clone())).copied().unwrap_or_default();
            // (AUDIT-B F-SHIP) on a form weapon's own bone: the notify's own attach data (LocationOffset in the bone's frame,
            // RotationOffset, bAbsoluteRotation, ToAttach, the FADEOUT time), carried by the spawn and its anchor's flags
            // (AUDIT-B F-SHIP) one of HIS sockets with attach-data flags (`vfx_center:free`: an ead row's ToAttach false, as the
            // warp's ead_pc_aura_com0 groups): the socket alone, its flags on the placement
            let mut socket = socket;
            if let (Some((bare, flags)), false) = (socket.split_once(':').filter(|_| !socket.starts_with("kb:")), trail) {
                let bare = bare.to_string();
                for flag in flags.split(':') {
                    match flag {
                        "free" => placement.follow = false,
                        "abs" => placement.absolute_rotation = true,
                        _ => {}
                    }
                }
                if let Anchor::Sora(_, offset) = &anchor {
                    anchor = Anchor::Sora(Some(bare.clone()), *offset);
                }
                socket = bare;
            }
            if let (Some(k), false) = (kbweapon::parse(&socket), trail) {
                let offset = match &anchor {
                    Anchor::Sora(_, o) => *o,
                    _ => Vec3::ZERO,
                };
                placement = Placement { offset, rotation: spawn.rotation, scale: Vec3::splat(spawn.scale), follow: !k.free, absolute_rotation: k.absolute, fade_out: k.fade_out, ..Placement::default() };
                anchor = Anchor::Sora(Some(socket.clone()), Vec3::ZERO);
            }
            // (FINISH-AUDIT2) a row of a WEAPON object (ATTACH_TO_WEAPON / _RIGHT_HAND_ / _LEFT_HAND_WEAPON): on that weapon
            // (the form's unit, else the Keyblade in his hand), its placement (LocationOffset ...) in that socket's frame
            let mut sent = None;
            if !trail {
                if let Anchor::Sora(at, offset) = &anchor {
                    let kh3 = weaponattach::decide(&file, at.as_deref(), s);
                    let sent_at = at.clone().unwrap_or_else(|| "None".to_string());
                    sent = Some((file.clone(), sent_at.clone(), kh3.clone().unwrap_or(sent_at)));
                    if let Some(name) = kh3.filter(|_| !weaponattach::OFF.load(std::sync::atomic::Ordering::Relaxed)) {
                        anchor = Anchor::Sora(Some(name), *offset);
                    }
                }
            }
            // (DON-KIT) a party member's spawn keeps its own rotation and scale (EffectSpawn, as Attach::World does; kh-app
            // vfx.rs, the same rule)
            if matches!(anchor, Anchor::Party(..)) {
                placement.rotation += spawn.rotation;
                placement.scale *= spawn.scale;
            }
            // (AUDIT-B W-FIN) the spawn's own attach-row timing (`Effects::timing`: a form weapon's / pawn's clip row): FadeIn,
            // StartDelayTime (0x141772CD0 -> FadeIn(t) / component +0xB88) and the FADEOUT end type's time, over the table's
            if let Some(t) = self.timing_next.filter(|t| !t.is_zero() && !trail) {
                if t.fade_in > 0.0 {
                    placement.fade_in = t.fade_in;
                }
                if t.start_delay > 0.0 {
                    placement.start_delay = t.start_delay;
                }
                if t.fade_out > 0.0 {
                    placement.fade_out = Some(t.fade_out);
                }
            }
            if trail {
                // ead_key_trail0: FADEOUT 0.15 (spec 6).
                placement.fade_out = Some(0.15);
            }
            let mut live = Live { handle, id: kh_vfx::EffectId(0), anchor, placement, trail, ends_with_animation: false, stopped: false, persistent: spawn.persistent, party_anim: None, inst: self.inst_now /* (MIMIC-FX) */, sent /* (FINISH-AUDIT2) */, owner: None /* (FREEZE-FX) */, equipment: false };
            let Some(xform) = self.anchor_xform(&live, s) else { continue };
            let options = SpawnOptions { seed: None, color: placement.color, start_delay: placement.start_delay, fade_in: placement.fade_in };
            let Some(id) = self.world.spawn(&file, xform, options) else { continue };
            live.id = id;
            // (INT) ON ONE OF HIS SOCKETS: where that socket was at the step before (the effect's first step is drawn between
            // the two, as the bone is; a jump of more than 3 m is a discontinuity, drawn without a blend anyway)
            let prev = match (&live.anchor, self.int_before.as_ref()) {
                (Anchor::Sora(..), Some((out_before, tap_before))) if tap_before.has_pose() => {
                    let before = Scene { out: out_before, sora: s.sora, tap: tap_before, target: s.target, ground: s.ground, eye: s.eye };
                    self.anchor_xform(&live, &before).filter(|p| p.position.distance(xform.position) < 300.0)
                }
                _ => None,
            };
            if let Some(prev) = prev {
                self.world.set_previous_transform(id, prev);
            }
            if self.slash_follow && is_slash_arc(&file) {
                live.placement.follow = true;
            }
            if !live.placement.follow && !trail && !matches!(live.anchor, Anchor::Projectile) {
                // `ToAttach` false: spawned in the world at the socket, then left there.
                // (FREEZE-FX) a pawn's own bad-state row keeps its owner for its material drives
                if pawn_bad_state_row(spawn.id.0) && matches!(live.anchor, Anchor::Sora(..) | Anchor::Party(..) | Anchor::Target(..)) {
                    live.owner = Some(live.anchor.clone());
                    live.equipment = (kh_core::combat::badstat::effect::ICE..=kh_core::combat::badstat::effect::ICE + 4).contains(&spawn.id.0);
                }
                live.anchor = Anchor::World(xform);
            }
            live.ends_with_animation = !spawn.persistent && self.world.def(&file).is_some_and(EffectDef::endless);
            // (ER party S5) a party member's endless one-shot ends with HIS anim-set row (kh-app vfx.rs party_anim)
            if let (true, Some(member)) = (live.ends_with_animation, kh_core::host::party::handle_member(handle.0)) {
                live.party_anim = Some((member.0, self.party_members.get(&member.0).map(|m| m.anim.clone()).unwrap_or_default()));
            }
            self.spawned += 1;
            self.live.push(live);
        }
    }

    fn stop(&mut self, handle: EffectHandle, immediate: bool) {
        self.kept_drives.retain(|k| k.0 != handle); // (FREEZE-FX) its UseLastParameter drives end with it
        for live in self.live.iter_mut().filter(|l| l.handle == handle) {
            live.stopped = true;
            match (immediate, live.placement.fade_out) {
                (true, _) => self.world.stop(live.id, true),
                (false, Some(seconds)) => self.world.fade_out(live.id, seconds),
                (false, None) => self.world.stop(live.id, false),
            }
        }
    }

    /// (AUDIT-C SW) How many particle systems the host started for this spawn handle (an attach data asset starts one per
    /// row, under the spawn's handle; a spawn whose effect the pack lacks starts none).
    pub fn drawn_of(&self, handle: EffectHandle) -> usize {
        self.live.iter().filter(|l| l.handle == handle).count()
    }

    /// (IMPL_ER75) A discontinuity (a teleport, the world shifted under him, an outfit or form change): the live trails drop
    /// their history; the next samples start at the blade.
    pub fn cut_trails(&mut self) {
        let ids: Vec<kh_vfx::EffectId> = self.live.iter().filter(|l| l.trail).map(|l| l.id).collect();
        for id in ids {
            self.world.clear_trail(id);
        }
        self.trail_stats.cuts += 1;
    }

    /// (IMPL_ER75) The trail numbers since the last call.
    pub fn take_trail_stats(&mut self) -> TrailStats {
        std::mem::take(&mut self.trail_stats)
    }

    /// Everything that is alive ends at once (Sora starts again, the effects are switched off).
    pub fn clear(&mut self) {
        for live in self.live.drain(..) {
            self.world.stop(live.id, true);
        }
        self.kept_drives.clear(); // (FREEZE-FX)
        self.rail_pieces.clear();
        self.rail_last = None;
        self.last_handle = 0;
        // (MIMIC-FX) the Sora instances' next cores count from 1; their Keyblades stay known
        for i in self.instances.values_mut() {
            i.restart();
        }
        // (prize / savepoint)
        for id in self.free.drain(..) {
            self.world.stop(kh_vfx::EffectId(id), true);
        }
    }

    /// ONE SIMULATION STEP: the seam's commands of the step, then the anchors of the step's pose,
    /// then one evaluator step.
    pub fn step(&mut self, dt: f32, commands: &[EffectCommand], s: &Scene) {
        self.right_weapon = right_weapon_of(&s.sora.combat().form); // (HAIR-PULSE)
        // (MIMIC-FX) a Sora instance's commands are carried out by HIS step (`step_instance`, on his scene): skipped here
        let kept: Vec<EffectCommand>;
        let commands = if !self.instances.is_empty() && commands.iter().any(|c| self.instance_owned(c)) {
            kept = commands.iter().filter(|c| !self.instance_owned(c)).copied().collect();
            &kept[..]
        } else {
            commands
        };
        for at in 0..commands.len() {
            self.apply_command(at, commands, s);
        }
        // An animation change ends the endless effects its one-shot notifies started.
        let play_id = s.out.anim.play_id;
        if play_id != self.play_id {
            self.play_id = play_id;
            for live in self.live.iter().filter(|l| l.ends_with_animation && l.handle.0 < kh_core::host::party::PARTY_HANDLE_BASE) {
                self.world.stop(live.id, false);
            }
        }
        // (ER party S5) a party member's anim-set row change ends the endless effects its one-shot notifies started
        for live in self.live.iter_mut().filter(|l| l.ends_with_animation && l.party_anim.is_some()) {
            let Some((member, anim)) = live.party_anim.as_ref() else { continue };
            if self.party_members.get(member).is_none_or(|m| &m.anim != anim) {
                self.world.stop(live.id, false);
                live.ends_with_animation = false;
            }
        }
        self.world.set_ground(s.ground);
        let mut k = 0;
        while k < self.live.len() {
            if !self.world.is_alive(self.live[k].id) {
                self.live.swap_remove(k);
                continue;
            }
            // (MIMIC-FX) a Sora instance's effects were placed on HIS scene by his own step
            if self.live[k].inst.is_none() {
                self.place_live(k, s);
            }
            k += 1;
        }
        self.world.step(dt);
        self.seconds += dt;
        self.remember_step(s);
        // (IMPL_ER75) the trails after the step: the newest sample against the blade's tip now
        let tip = self.socket_world(s, Some("vfx_top")).position;
        for live in self.live.iter().filter(|l| l.trail && !l.stopped && l.inst.is_none()) {
            if let Some((head, samples)) = self.world.trail_head(live.id) {
                let t = &mut self.trail_stats;
                t.steps += 1;
                t.max_gap = t.max_gap.max(head.distance(tip));
                t.max_samples = t.max_samples.max(samples);
                t.dt = dt;
            }
        }
        // SQEX_CameraShake (0x1442FC5E0): the scale falls with the camera's distance from the node.
        for request in self.world.take_shakes() {
            let scale = request.scale * distance_scale(s.eye.distance(request.position), request.distance[0], request.distance[1], request.distance[2]);
            if self.shakes.len() < 32 {
                self.shakes.push(Shake { class: request.class, scale, duration: request.duration });
            }
        }
        for line in self.world.take_log() {
            self.say(line);
        }
    }

    /// (INT) This step's place and bones are "the step before" of the next one's spawns (MIMIC-FX: cut out of `step`; a
    /// Sora instance's step keeps his own).
    fn remember_step(&mut self, s: &Scene) {
        match self.int_before.as_mut() {
            Some((out, tap)) => {
                (out.position, out.yaw) = (s.out.position, s.out.yaw);
                tap.copy_latest_from(s.tap);
            }
            None => {
                let mut tap = PoseTap::default();
                tap.copy_latest_from(s.tap);
                self.int_before = Some((s.out.clone(), tap));
            }
        }
    }

    /// One live effect placed on scene `s` for the coming evaluator step: its anchor, its bones, its beam ends, its trail
    /// (MIMIC-FX: cut out of `step`; a Sora instance's are placed on HIS scene).
    fn place_live(&mut self, k: usize, s: &Scene) {
        let (id, trail, projectile) = (self.live[k].id, self.live[k].trail && !self.live[k].stopped, matches!(self.live[k].anchor, Anchor::Projectile));
        match self.anchor_xform(&self.live[k], s) {
            Some(xform) => self.world.set_transform(id, xform),
            // The spell actor is gone: its body effect runs out where it was.
            None if projectile => self.world.stop(id, false),
            None => {}
        }
        // (ZETTA) SQEX_BoneAttach: an effect on a party member sits on his bones (kh-app vfx.rs, the same rule)
        if let Anchor::Party(member, ..) = self.live[k].owner.as_ref().unwrap_or(&self.live[k].anchor) /* (FREEZE-FX) */ {
            if self.world.wants_bones(id) {
                let bones = self.party_members.get(member).map(|m| std::sync::Arc::new(m.bones.clone()));
                self.world.set_bones(id, bones);
            }
        }
        // (FREEZE-FX) the particles that start on the victim's mesh (KH3's frozen look): an enemy's own bones (the host's read of
        // them), his on Sora's body (not on a weapon's socket: that mesh is the weapon's)
        match self.live[k].owner.as_ref().unwrap_or(&self.live[k].anchor) {
            Anchor::Target(t, _) if self.world.wants_bones(id) => {
                let bones = self.target_bones.get(&t.0).cloned();
                self.world.set_bones(id, bones);
            }
            Anchor::Sora(socket, _) if !socket.as_deref().is_some_and(|b| b.starts_with("kb:") || b.starts_with("wep:")) && s.tap.has_pose() && self.world.wants_bones(id) => {
                self.world.set_bones(id, Some(std::sync::Arc::new(s.tap.bones_world())));
            }
            _ => {}
        }
        // (AUDIT-B W-VFX) TypeDataSQEX_Thunder: KH3's tick (0x144250690) reads the beam ends from the mesh the component
        // hangs on, every tick: the attach row's SocketName / EndSocketName, or the bone list's links
        for (emitter, ends) in self.beam_ends(&self.live[k], id, s) {
            self.world.set_beam_ends(id, emitter, ends);
        }
        if trail {
            // (AUDIT-B F-SHIP) a form weapon's trail between ITS sockets
            let (top, bottom) = self.trail_points(&self.live[k], s);
            self.world.set_trail(id, top, bottom);
        }
    }

    /// One command of a step's list (`at` its index: a spawn's row timing follows it) on scene `s` (MIMIC-FX: cut out of
    /// `step`; a Sora instance's step carries out his own on HIS scene).
    fn apply_command(&mut self, at: usize, commands: &[EffectCommand], s: &Scene) {
        {
            let command = &commands[at];
            match command {
                // (AUDIT-B W-FIN) applied with its spawn (below); a later one only changes the FADEOUT time of what is alive
                EffectCommand::Timing(handle, t) => {
                    if !commands[..at].iter().any(|c| matches!(c, EffectCommand::Spawn(h, _) if h == handle)) && t.fade_out > 0.0 {
                        for live in self.live.iter_mut().filter(|l| l.handle == *handle && !l.trail) {
                            live.placement.fade_out = Some(t.fade_out);
                        }
                    }
                }
                EffectCommand::Spawn(handle, spawn) => {
                    // (AUDIT-B W-FIN) the row timing sent right after this spawn
                    self.timing_next = commands[at + 1..].iter().find_map(|c| match c {
                        EffectCommand::Timing(h, t) if h == handle => Some(*t),
                        _ => None,
                    });
                    // (ER party S5) only Sora's handles count from 1: the party's own ranges (kh_core::host::party)
                    // take no part in the "new core" check, and a new core of Sora's leaves the party's effects alone
                    // (before: the next spawn of Sora's after one of the party's ended every effect alive)
                    // (a host's own handles above the party ranges, e.g. the unlock celebration's burst 0x7E00_0000, neither)
                    if handle.0 < kh_core::host::party::PARTY_HANDLE_BASE {
                        if handle.0 <= self.last_handle {
                            // A new core counts its handles from 1 again: the old one's effects have no owner.
                            self.kept_drives.retain(|k| k.0 .0 >= kh_core::host::party::PARTY_HANDLE_BASE); // (FREEZE-FX)
                            let world = &mut self.world;
                            self.live.retain(|live| {
                                let party = live.handle.0 >= kh_core::host::party::PARTY_HANDLE_BASE;
                                if !party {
                                    world.stop(live.id, true);
                                }
                                party
                            });
                        }
                        self.last_handle = handle.0;
                    }
                    self.start(*handle, spawn, s);
                    self.timing_next = None; // (AUDIT-B W-FIN)
                }
                EffectCommand::Stop(handle, immediate) => self.stop(*handle, *immediate),
                // (DON-MAGIC tint) the plan's [R] recolours: every system of the handle
                EffectCommand::Tint(handle, tint) => {
                    for live in self.live.iter().filter(|l| l.handle == *handle) {
                        self.world.set_tint(live.id, Some([tint.color[0], tint.color[1], tint.color[2], tint.intensity]));
                    }
                }
                // (AUDIT-C) the attach row's ParticleColor / ParticleAlpha: a multiplier of everything the systems draw
                EffectCommand::Color(handle, rgba) => {
                    for live in self.live.iter().filter(|l| l.handle == *handle) {
                        self.world.set_color(live.id, *rgba);
                    }
                }
                // (ZETTA) Zettaflare's beam laid along the staff -> target line: a world anchor is moved, turned and scaled; an
                // attached one keeps its anchor and takes the turn and the scale
                EffectCommand::Transform(handle, at, rotation, scale) => {
                    for live in self.live.iter_mut().filter(|l| l.handle == *handle) {
                        match &mut live.anchor {
                            Anchor::World(x) => {
                                x.position = *at;
                                x.rotation = kh_vfx::rotator(rotation.x, rotation.y, rotation.z);
                                x.scale = *scale;
                            }
                            Anchor::Projectile => {}
                            _ => {
                                live.placement.rotation = *rotation;
                                live.placement.scale = *scale;
                            }
                        }
                    }
                }
                // (DON-MAGIC orbit) Ball Lightning's balls: a world anchor moves to the point, an attached one takes it as offset
                EffectCommand::Place(handle, at) => {
                    for live in self.live.iter_mut().filter(|l| l.handle == *handle) {
                        match &mut live.anchor {
                            Anchor::World(x) => x.position = *at,
                            Anchor::Sora(_, offset) | Anchor::Target(_, offset) | Anchor::Party(_, _, offset) => *offset = *at,
                            Anchor::Projectile => {}
                        }
                    }
                }
            }
        }
    }

    /// The camera shakes the effects asked for since the last call (A REQUEST to the host).
    pub fn take_shakes(&mut self) -> Vec<Shake> {
        std::mem::take(&mut self.shakes)
    }

    /// The scene's light scale the effects ask for (`SQEX_ChangeSceneColor.LightIntensity`;
    /// 1 = untouched). A REQUEST to the host; the test app squares it (calibrated on a capture).
    pub fn scene_light(&self) -> f32 {
        self.world.scene_light()
    }

    /// (systems alive, emitters, particles; persistent and not yet stopped by the core; ending).
    pub fn stats(&self) -> (usize, usize, usize, usize, usize) {
        let (systems, emitters, particles) = self.world.stats();
        (systems, emitters, particles, self.live.iter().filter(|l| l.persistent && !l.stopped).count(), self.live.iter().filter(|l| l.stopped).count())
    }

    /// The draw list of a frame `alpha` of the way from the step before to the latest one, for
    /// `camera`, with the Blizzard ice trail's pieces as mesh instances; and the names behind the
    /// list's material and mesh ids (index = id).
    pub fn draw(&mut self, alpha: f32, camera: &kh_vfx::Camera) -> (&DrawList, &[String], &[String]) {
        let mut list = std::mem::take(&mut self.list);
        list.clear();
        self.world.draw(alpha, camera, &mut list);
        let now = self.seconds;
        self.rail_pieces.retain(|p| now - p.born < RAIL_LIFE);
        if let Some(rail) = &self.rail {
            for piece in &self.rail_pieces {
                let (mesh, material_name, length) = &rail.segments[piece.segment];
                let along = piece.to - piece.from;
                let dir = along.normalize_or(Vec3::X);
                let side = Vec3::Z.cross(dir).normalize_or(Vec3::Y);
                let up = dir.cross(side);
                // It melts away after the fade-out wait: the cross-section shrinks to nothing.
                let age = now - piece.born;
                let melt = if age > rail.fade_wait { (1.0 - (age - rail.fade_wait) / (RAIL_LIFE - rail.fade_wait).max(1.0e-3)).clamp(0.0, 1.0) } else { 1.0 };
                let stretch = along.length() / length.max(1.0);
                let m = Mat3::from_cols(dir * stretch, side * piece.scale.x * melt, up * piece.scale.y * melt);
                let (mesh, material) = (self.world.intern_mesh(mesh), self.world.intern_material(material_name));
                list.meshes.push(kh_vfx::MeshInstance { mesh, material, transform: Affine3A::from_mat3_translation(m, piece.from + Vec3::Z * rail.offset), color: [1.0; 4], dynamic: [0.0; 4], time: age, rel: age / RAIL_LIFE, blend: kh_vfx::Blend::Material, uv_mode: 0.0, sub: 0.0 /* (ZA) */ });
            }
        }
        self.list = list;
        // (FREEZE-FX) the UseLastParameter drives of persistent spawns not stopped: kept past their emitter's end
        for (effect, keep, d) in self.fresh_drives() {
            if !keep {
                continue;
            }
            let Some(handle) = self.live.iter().find(|l| l.id.0 == effect && l.persistent && !l.stopped).map(|l| l.handle) else { continue };
            self.kept_drives.retain(|k| !(k.1.owner == d.owner && k.1.name == d.name && k.1.mesh == d.mesh));
            self.kept_drives.push((handle, d));
        }
        while let Some(name) = self.world.material_name(self.material_names.len() as u32) {
            self.material_names.push(name.to_string());
        }
        while let Some(name) = self.world.mesh_name(self.mesh_names.len() as u32) {
            self.mesh_names.push(name.to_string());
        }
        (&self.list, &self.material_names, &self.mesh_names)
    }
}

/// The distance term of `SQEX_CameraShake` (0x1442FC5E0 [C]; as kh_camera::shake::distance_scale):
/// with `min < 1` the scale falls linearly from 1 at `offset` cm between the camera and the
/// emitter node to `min` at `distance`.
pub fn distance_scale(camera_to_node: f32, distance: f32, offset: f32, min: f32) -> f32 {
    if min >= 1.0 {
        return 1.0;
    }
    let d = (camera_to_node - offset).max(0.0) / (distance - offset).max(1.0e-6);
    (min - 1.0) * d.min(1.0) + 1.0
}

/// (prize / savepoint) FREE EFFECTS: particle systems of the pack that a part of the host owns
/// (a prize orb, a save point), not the core: no handle of the seam, no anchor; whoever started
/// one moves and stops it. They are stepped and drawn with everything else. (The same four calls
/// as the test app's `Vfx::free_*`.)
impl EffectHost {
    /// (INT) A DISCONTINUITY of what the effects hang on (a teleport, a reseat, a warp): drawn at the latest step until
    /// the next one (`kh_vfx::World::snap_draw`).
    pub fn snap(&mut self) {
        self.world.snap_draw();
    }

    /// (INT) WHERE HE IS DRAWN THIS FRAME (`s`: a scene of his DRAWN pose and place, between the steps): every live effect
    /// on one of his sockets / bones is drawn on that socket as drawn (its emitters between their places in it at the two
    /// steps), the Keyblade trail's head at the drawn blade (`kh_vfx::World::set_drawn`). For the next `draw`; then
    /// `clear_drawn`.
    pub fn set_drawn(&mut self, s: &Scene) {
        let mut sets = Vec::new();
        // (MIMIC-FX) the player's only: a Sora instance's are drawn between his own steps' places
        for l in self.live.iter().filter(|l| matches!(l.anchor, Anchor::Sora(..)) && l.inst.is_none()) {
            let trail = (l.trail && !l.stopped).then(|| self.trail_points(l, s)); // (AUDIT-B F-SHIP) its own weapon's sockets
            sets.push((l.id, self.anchor_xform(l, s), trail));
        }
        for (id, component, trail) in sets {
            self.world.set_drawn(id, component, trail);
        }
    }

    /// (INT) The drawn places of `set_drawn` are forgotten (after the draw).
    pub fn clear_drawn(&mut self) {
        self.world.clear_drawn();
    }

    /// (INT, measuring aid) The live effects on Sora's sockets / bones and the component each would be given for scene
    /// `s` (a scene of his DRAWN pose gives where they belong in the frame).
    pub fn sora_anchored(&self, s: &Scene) -> Vec<(kh_vfx::EffectId, Xform)> {
        self.live.iter().filter(|l| matches!(l.anchor, Anchor::Sora(..)) && !l.stopped && l.inst.is_none() /* (MIMIC-FX) */).filter_map(|l| Some((l.id, self.anchor_xform(l, s)?))).collect()
    }

    /// (INT, measuring aid) A socket / bone of his (or of the weapon) in the world for scene `s` (`vfx_top`: the blade's tip).
    pub fn socket_point(&self, s: &Scene, name: &str) -> Vec3 {
        self.socket_world(s, Some(name)).position
    }

    /// (AUDIT-B F-SHIP) A trail's two blade points: the hand keyblade's vfx_top / vfx_bottom, or (a form weapon's trail,
    /// anchored `kb:<model>:<mount>:vfx_top`) that weapon's own.
    fn trail_points(&self, live: &Live, s: &Scene) -> (Vec3, Vec3) {
        if let Anchor::Sora(Some(top), _) = &live.anchor {
            if let Some(bottom) = kbweapon::with_bone(top, "vfx_bottom") {
                return (self.socket_world(s, Some(top)).position, self.socket_world(s, Some(&bottom)).position);
            }
        }
        (self.socket_world(s, Some("vfx_top")).position, self.socket_world(s, Some("vfx_bottom")).position)
    }

    /// (INT, measuring aid) The live Keyblade trails (world ids).
    pub fn trail_ids(&self) -> Vec<kh_vfx::EffectId> {
        self.live.iter().filter(|l| l.trail && !l.stopped).map(|l| l.id).collect()
    }

    /// Starts effect `name` at a world position (core cm) with a uniform scale; `None` when the
    /// pack has no such effect.
    pub fn free_spawn(&mut self, name: &str, position: Vec3, yaw: f32, scale: f32) -> Option<u64> {
        let file = self.ensure(name)?;
        let xform = Xform { position, rotation: kh_vfx::rotator(0.0, yaw, 0.0), scale: Vec3::splat(scale) };
        let id = self.world.spawn(&file, xform, SpawnOptions::default())?;
        // (IMPL_ER56) the free effects (the save point, the prizes) are AMBIENT effects: their lights light Sora with a
        // small share only (kh-er-render pointlight `fx.ambient_sora`; the user: "Sora looks HORRIBLE with the blue light",
        // then "I want ambient effects to light Sora")
        self.world.set_lights_on_characters(id, false);
        self.spawned += 1;
        self.free.retain(|f| self.world.is_alive(kh_vfx::EffectId(*f)));
        self.free.push(id.0);
        Some(id.0)
    }

    pub fn free_move(&mut self, id: u64, position: Vec3, yaw: f32, scale: f32) {
        self.world.set_transform(kh_vfx::EffectId(id), Xform { position, rotation: kh_vfx::rotator(0.0, yaw, 0.0), scale: Vec3::splat(scale) });
    }

    /// `immediate`: gone at once; else it stops emitting and its particles run out.
    pub fn free_stop(&mut self, id: u64, immediate: bool) {
        self.world.stop(kh_vfx::EffectId(id), immediate);
    }

    /// (IMPL_ER19) A factor on everything the effect draws from now on (its colour multiplier; alpha untouched).
    pub fn free_gain(&mut self, id: u64, gain: f32) {
        let gain = gain.clamp(0.0, 100.0);
        self.world.set_color(kh_vfx::EffectId(id), [gain, gain, gain, 1.0]);
    }

    pub fn free_alive(&self, id: u64) -> bool {
        self.world.is_alive(kh_vfx::EffectId(id))
    }

    /// (LDI) A free effect as if it had been running for `seconds` (kh_vfx `World::prewarm`): a looping idle effect made
    /// while the screen is black (a load) or made again where it stood is drawn full at once, not filling up.
    pub fn free_prewarm(&mut self, id: u64, seconds: f32) {
        self.world.prewarm(kh_vfx::EffectId(id), seconds);
    }

    /// (LDI) Parses these pack effects now (on the loader's thread), so their materials and meshes are among `assets` and
    /// the renderer makes them ready before the first spawn (the save point's five systems). Returns how many are known.
    pub fn preload_names(&mut self, names: &[&str]) -> usize {
        names.iter().filter(|n| self.ensure(n).is_some()).count()
    }
}

// ------------------------------------------------------------------------------------ (VFXR) character drives

/// (VFXR) One character-material drive of the frame (`SQEX_Material_InLight / _Aura / _Scalar / _Vector`), with the
/// character it acts on: `owner` 0 = Sora, 1 + m = party member m (`kh_core::PartyMember.0`: 1 Donald, 2 Goofy), as the
/// effect's anchor says.
/// `InLightCenterPosition` is relative to that character's root (world axes, cm: kh_vfx::screen::in_light_center).
#[derive(Clone, Debug, PartialEq)]
pub struct CharDrive {
    pub owner: u8,
    pub name: String,
    pub value: [f32; 4],
    pub vector: bool,
    pub target: u8,
    /// (AUDIT-B W-VFX) the form weapon / pawn model the effect hangs on (its `kb:` anchor), None: the owner's body. KH3's
    /// SQEX_Material_Scalar / _Vector write the mesh the component is attached to (0x1442871A0).
    pub mesh: Option<String>,
    /// (FREEZE-FX) The drive also reaches every weapon the character holds (KH3's equipment component: `Live::equipment`).
    pub equipment: bool,
}

impl EffectHost {
    /// (VFXR) The latest draw list's character drives, owner resolved (`draw` first). (FREEZE-FX) With the UseLastParameter
    /// drives kept past their emitter's end (`kept_drives`) where no fresh drive of the same owner, parameter and mesh is.
    pub fn char_drives(&self) -> Vec<CharDrive> {
        let mut out: Vec<CharDrive> = self.fresh_drives().into_iter().map(|d| d.2).collect();
        for (_, k) in &self.kept_drives {
            if !out.iter().any(|d| d.owner == k.owner && d.name == k.name && d.mesh == k.mesh) {
                out.push(k.clone());
            }
        }
        out
    }

    /// (FREEZE-FX) The latest draw list's own drives, owner resolved: (the effect's id, UseLastParameter, the drive).
    fn fresh_drives(&self) -> Vec<(u64, bool, CharDrive)> {
        let sora_root = self.int_before.as_ref().map_or(Vec3::ZERO, |(o, _)| o.position);
        self.list
            .char_params
            .iter()
            // (HAIR-PULSE) KH3 writes a SQEX_Material_* drive on the mesh its component was ATTACHED to (0x1442871A0's last
            // case: the component's "AttachOwnerMeshComp" instance parameter, set by the SQEX attach only; InLight's 0x14437C8C0
            // the same through vt+0x2F8): an effect placed in the world (SpawnEmitterAtLocation, or a weapon's own clip row the
            // core places by hand: Blizzard Claws' transform unit w_so081's vw_so082_cng_mat0 aura), on an enemy or on a spell
            // actor drives no character; only an effect hung on Sora (his sockets, his weapons' `kb:` sockets) or a party member
            // (FREEZE-FX) a pawn's own bad-state row left in the world drives its owner (`Live::owner`)
            .filter(|p| self.live.iter().find(|l| l.id.0 == p.effect).is_none_or(|l| matches!(l.owner.as_ref().unwrap_or(&l.anchor), Anchor::Sora(..) | Anchor::Party(..))))
            .map(|p| {
                let live = self.live.iter().find(|l| l.id.0 == p.effect);
                let anchor_of = |l: &Live| -> Anchor { l.owner.clone().unwrap_or_else(|| l.anchor.clone()) }; // (FREEZE-FX)
                // (MIMIC-FX) a Sora instance's effect drives HIM: owner = his id + 1
                let inst = live.and_then(|l| l.inst);
                let owner = live.map_or(0, |l| if let Some(m) = l.inst { m.saturating_add(1) } else if let Anchor::Party(m, ..) = anchor_of(l) { m.saturating_add(1) } else { 0 });
                // (AUDIT-B W-VFX) on a form weapon / pawn: its model
                let kb = live.and_then(|l| if let Anchor::Sora(Some(socket), _) = anchor_of(l) { kbweapon::parse(&socket).map(|k| k.model.to_string()) } else { None });
                // (HAIR-PULSE) a SUB in-light drive (ParamType SQEX_IN_LIGHT_TYPE_SUB) of an effect on one of HIS sockets is the
                // form charge glow vs_com_lv2, which KH3 hangs on his right-hand weapon (vfx spec 2.8, 0x141792220: group 3001 on
                // the weapon; the core puts it on that weapon's socket, R_wep_skt ...): that weapon's materials read it (their
                // `Sub_InLight*`); his body's never do. Every SUB module of the pack sits on the right-hand weapon
                // (vw_so093_fns_ora0: ATTACH_TO_RIGHT_HAND_WEAPON, a `kb:` anchor already).
                let sora_socket = live.is_some_and(|l| matches!(anchor_of(l), Anchor::Sora(..)));
                // (MIMIC-FX) a Sora instance's right-hand weapon is HIS
                let right = match inst {
                    Some(m) => self.instances.get(&m).and_then(|i| i.right_weapon.clone()),
                    None => self.right_weapon.clone(),
                };
                let mesh = kb.or_else(|| (p.target == 1 && (owner == 0 || inst.is_some()) && sora_socket).then_some(right).flatten());
                let mut value = p.value;
                if kh_vfx::screen::is_in_light_center(&p.name) && mesh.is_none() {
                    // (HAIR-PULSE) was == "InLightCenterPosition": the SUB module's Sub_InLightCenterPosition too
                    let root = match inst {
                        Some(m) => self.instances.get(&m).map_or(sora_root, instances::SoraInst::root), // (MIMIC-FX) his own root
                        None if owner == 0 => sora_root,
                        None => self.party_members.get(&(owner - 1)).map_or(sora_root, |m| m.position),
                    };
                    value = kh_vfx::screen::in_light_center(Vec3::new(value[0], value[1], value[2]), root);
                }
                // (HAIR-PULSE) on a weapon the centre stays in the world (w 1, as kh_vfx samples it): a weapon material measures it
                // from ITS OWN object position (Primitive ObjectWorldPosition, `i.p8 = kh.obj`, w_so200_mat0_Inst KH_C554 lines
                // 197-230), which the renderer knows per slot at its draw (kh-er-render vfx.rs `weapon_in_light`)
                let equipment = live.is_some_and(|l| l.equipment); // (FREEZE-FX)
                (p.effect, p.keep, CharDrive { owner, name: p.name.clone(), value, vector: p.vector, target: p.target, mesh, equipment })
            })
            .collect()
    }
}

/// (HAIR-PULSE) His right-hand weapon's model in the held form: the `RWeapon` unit at its grow level
/// (`RWeapon@RIGHT_HAND:R_wep_skt=/Game/Blueprints/Player/p_ex001/w_so200_Wep` -> `w_so200`); None out of a form.
fn right_weapon_of(form: &kh_core::combat::form::Form) -> Option<String> {
    // (the blueprints are `w_so200_Wep`, the shields' `w_so032_wep`)
    let bare = |b: &str| (b.len() > 4 && b[b.len() - 4..].eq_ignore_ascii_case("_wep")).then(|| b[..b.len() - 4].to_string());
    form.weapon_units().iter().find(|u| u.starts_with("RWeapon@")).and_then(|u| u.rsplit('/').next()).and_then(bare)
}

#[cfg(test)]
mod tests {
    use super::four_digit_group;

    #[test]
    fn a_four_digit_weapon_group_is_its_three_digit_models() {
        assert_eq!(four_digit_group("vw_so0033_lv1").as_deref(), Some("vw_so033_lv1"));
        assert_eq!(four_digit_group("vw_so0034_lv2").as_deref(), Some("vw_so034_lv2"));
        assert_eq!(four_digit_group("vw_so033_lv1"), None);
        assert_eq!(four_digit_group("vw_so010_app0"), None);
        assert_eq!(four_digit_group("vw_so0x3_lv1"), None);
    }

    /// (SHOT-FIX) Atomic Deluge's spark rays (M_B481's AttachEffect: vw_so033_gsh_spk01 on vfx_root, LocationOffset
    /// (0, 0, 245), bAbsoluteRotation) are placed by their notify's own attach data: 245 cm up, in the world's rotation.
    #[test]
    fn the_shotlock_clips_place_their_effects_by_their_notifies() {
        let rows = super::notify_placements();
        let (_, spk) = rows.iter().find(|(k, _)| k.0 == "vw_so033_gsh_spk01" && k.1 == "vfx_root").expect("the spark row of M_B481");
        assert_eq!((spk.offset, spk.rotation, spk.absolute_rotation), (glam::Vec3::new(0.0, 0.0, 245.0), glam::Vec3::ZERO, true));
    }

    /// (VFX-2) An attach-data asset spawned with a group resolves to that group's file (`<asset>__g<group>`) when the pack
    /// has one, else to the asset's own file: Snakebite's charge ead_shf_tame0 exists only as `__g1835`.
    #[test]
    fn an_attach_data_spawn_takes_its_groups_file_first() {
        let pack = std::env::temp_dir().join(format!("kh_host_vfx2_group_{}", std::process::id()));
        let effects = pack.join("vfx").join("effects");
        std::fs::create_dir_all(&effects).unwrap();
        for name in ["ead_shf_tame0__g1835", "ead_so021_sht0", "ead_so021_sht0__g1822"] {
            std::fs::write(effects.join(format!("{name}.json")), format!(r#"{{"name":"{name}","emitters":[]}}"#)).unwrap();
        }
        let mut host = super::EffectHost::load(&pack).unwrap();
        let mut resolve = |name: &'static str, group: u32| {
            let spawn = kh_core::host::EffectSpawn { id: kh_core::host::EffectId(group), name, at: kh_core::host::Attach::Sora { bone: Some("R_wep_skt"), offset: glam::Vec3::ZERO }, rotation: glam::Vec3::ZERO, scale: 1.0, persistent: false };
            host.resolve_spawn(&spawn).into_iter().map(|(effect, ..)| effect).collect::<Vec<_>>()
        };
        assert_eq!(resolve("ead_shf_tame0", 1835), ["ead_shf_tame0__g1835"]);
        assert_eq!(resolve("ead_so021_sht0", 1822), ["ead_so021_sht0__g1822"]);
        // a group without its own file: the asset's
        assert_eq!(resolve("ead_so021_sht0", 7), ["ead_so021_sht0"]);
        std::fs::remove_dir_all(&pack).ok();
    }
}
