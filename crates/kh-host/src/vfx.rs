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
const XSI_ALLOWED_MAIN: [&str; 3] = ["vw_so200_spr_tub00", "vw_so072_stf_przm_one0", "vw_so072_stf_przm_one_hit0"];

/// (XW-d) `fx.xs_own` (default on; read before the XSI root is read): an XS effect that has a file in
/// `PACK/xsi/vfx/effects_xwd/` is read from there instead of `effects/` (the XS assets' own materials under the suffix
/// rule `name__<last 3 folders>` where the main pack has another asset of that name, the fragment pieces the root
/// lacked). Off: `effects/` only, as before.
pub static XS_OWN_FILES: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

/// (ER90) The XSI root's index (see `EffectHost::load_xsi_root`).
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
    /// (ER party S5) The party's second root (None until `load_party_root`: everything as before).
    party_root: Option<PartyRoot>,
    /// (ER party S5) The members as of the latest step, and the party's spell actors / team props (body-effect handle,
    /// position, flight direction).
    party_members: HashMap<u8, PartyMemberFx>,
    party_projectiles: Vec<(u32, Vec3, Vec3)>,
    /// (ER90) The XS abilities' root (None until `load_xsi_root`: everything as before).
    xsi_root: Option<XsiRoot>,
    /// (INT) His place, facing and bones as of the step BEFORE the latest (an effect spawned on one of his sockets is
    /// drawn from where that socket was then: `World::set_previous_transform`).
    int_before: Option<(Output, PoseTap)>,
}

/// (IMPL_ER75) A melee swing arc of the pack (`vp_qst_sho00`, `vp_fst_sho01`, `vp_rad_sho00` ...).
pub fn is_slash_arc(file: &str) -> bool {
    let lower = file.to_lowercase();
    lower.starts_with("vp_") && lower.contains("_sho")
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
            party_root: None,
            party_members: HashMap::new(),
            party_projectiles: Vec::new(),
            xsi_root: None,
            int_before: None,
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
        for roles in host.projectiles.values() {
            for list in roles.values() {
                wanted.extend(list.iter().map(|n| n.to_lowercase()));
            }
        }
        // (DON-KIT) THE ITEM EFFECTS (the Recover kinds: Potion, Hi-Potion, Ether, Elixir ... and the party Cure's body) are
        // preloaded too: the user, 2026-10-22, "the little item VFX doesn't appear when Goofy and Donald first use an item"
        // (their first spawn waited on its materials while the one-second effect ran out)
        wanted.extend(item_effect_names().into_iter().map(str::to_lowercase));
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
                    kh_vfx::Kind::AnimTrail | kh_vfx::Kind::Tail => add(&e.material, AssetGeo::Ribbon, e.blend),
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
        let (file, party, xsi) = match self.files.get(&lower) {
            Some(file) => (file.clone(), false, false),
            None => match self.party_root.as_ref().and_then(|r| r.files.get(&lower)) {
                Some(file) => (file.clone(), true, false),
                None => (self.xsi_root.as_ref()?.files.get(&lower)?.clone(), false, true),
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
        for live in self.live.iter_mut().filter(|l| kh_core::host::party::is_party_handle(l.handle.0) && !l.stopped) {
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
        let bone = |bone: &str| s.tap.bone_world(bone).map(|t| Xform { position: t.translation, rotation: Mat3::from_quat(t.rotation), scale: t.scale });
        let relative = |s: &Socket| Xform { position: s.location, rotation: kh_vfx::rotator(s.rotation.x, s.rotation.y, s.rotation.z), scale: Vec3::ONE };
        if let Some(socket) = self.sockets.get(name) {
            return bone(&socket.bone).map_or(component, |b| b.then(&relative(socket)));
        }
        if let Some(socket) = self.weapon_sockets.get(name) {
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
                let (position, forward) = match self.party_projectiles.iter().find(|p| p.0 == live.handle.0) {
                    Some(p) => (p.1, p.2),
                    None => s.sora.magic().projectile_views().find(|p| p.effect == Some(live.handle)).map(|p| (p.position, p.forward))?,
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
        if lower.starts_with("vw_so") && !lower.starts_with("vw_so010") && !lower.starts_with("vw_sor_") && !self.xsi_allowed(&lower) {
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
            out.push(("VW_SO010_LOC0".to_string(), Anchor::Sora(Some("vfx_top".to_string()), Vec3::ZERO), true));
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
            out.push((name.to_string(), anchor, false));
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
        for (effect, anchor, trail) in self.resolve_spawn(spawn) {
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
            let mut placement = self.placements.get(&(placed_as, socket)).copied().unwrap_or_default();
            // (DON-KIT) a party member's spawn keeps its own rotation and scale (EffectSpawn, as Attach::World does; kh-app
            // vfx.rs, the same rule)
            if matches!(anchor, Anchor::Party(..)) {
                placement.rotation += spawn.rotation;
                placement.scale *= spawn.scale;
            }
            if trail {
                // ead_key_trail0: FADEOUT 0.15 (spec 6).
                placement.fade_out = Some(0.15);
            }
            let mut live = Live { handle, id: kh_vfx::EffectId(0), anchor, placement, trail, ends_with_animation: false, stopped: false, persistent: spawn.persistent, party_anim: None };
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
        for live in self.live.iter_mut().filter(|l| l.handle == handle) {
            live.stopped = true;
            match (immediate, live.placement.fade_out) {
                (true, _) => self.world.stop(live.id, true),
                (false, Some(seconds)) => self.world.fade_out(live.id, seconds),
                (false, None) => self.world.stop(live.id, false),
            }
        }
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
        self.rail_pieces.clear();
        self.rail_last = None;
        self.last_handle = 0;
        // (prize / savepoint)
        for id in self.free.drain(..) {
            self.world.stop(kh_vfx::EffectId(id), true);
        }
    }

    /// ONE SIMULATION STEP: the seam's commands of the step, then the anchors of the step's pose,
    /// then one evaluator step.
    pub fn step(&mut self, dt: f32, commands: &[EffectCommand], s: &Scene) {
        for command in commands {
            match command {
                EffectCommand::Spawn(handle, spawn) => {
                    // (ER party S5) only Sora's handles count from 1: the party's own ranges (kh_core::host::party)
                    // take no part in the "new core" check, and a new core of Sora's leaves the party's effects alone
                    // (before: the next spawn of Sora's after one of the party's ended every effect alive)
                    // (a host's own handles above the party ranges, e.g. the unlock celebration's burst 0x7E00_0000, neither)
                    if handle.0 < kh_core::host::party::PARTY_HANDLE_BASE {
                        if handle.0 <= self.last_handle {
                            // A new core counts its handles from 1 again: the old one's effects have no owner.
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
                }
                EffectCommand::Stop(handle, immediate) => self.stop(*handle, *immediate),
                // (DON-MAGIC tint) the plan's [R] recolours: every system of the handle
                EffectCommand::Tint(handle, tint) => {
                    for live in self.live.iter().filter(|l| l.handle == *handle) {
                        self.world.set_tint(live.id, Some([tint.color[0], tint.color[1], tint.color[2], tint.intensity]));
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
            let (id, trail, projectile) = (self.live[k].id, self.live[k].trail && !self.live[k].stopped, matches!(self.live[k].anchor, Anchor::Projectile));
            match self.anchor_xform(&self.live[k], s) {
                Some(xform) => self.world.set_transform(id, xform),
                // The spell actor is gone: its body effect runs out where it was.
                None if projectile => self.world.stop(id, false),
                None => {}
            }
            // (ZETTA) SQEX_BoneAttach: an effect on a party member sits on his bones (kh-app vfx.rs, the same rule)
            if let Anchor::Party(member, ..) = &self.live[k].anchor {
                if self.world.wants_bones(id) {
                    let bones = self.party_members.get(member).map(|m| std::sync::Arc::new(m.bones.clone()));
                    self.world.set_bones(id, bones);
                }
            }
            if trail {
                let top = self.socket_world(s, Some("vfx_top")).position;
                let bottom = self.socket_world(s, Some("vfx_bottom")).position;
                self.world.set_trail(id, top, bottom);
            }
            k += 1;
        }
        self.world.step(dt);
        self.seconds += dt;
        // (INT) this step's place and bones are "the step before" of the next one's spawns
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
        // (IMPL_ER75) the trails after the step: the newest sample against the blade's tip now
        let tip = self.socket_world(s, Some("vfx_top")).position;
        for live in self.live.iter().filter(|l| l.trail && !l.stopped) {
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
        for l in self.live.iter().filter(|l| matches!(l.anchor, Anchor::Sora(..))) {
            let trail = (l.trail && !l.stopped).then(|| (self.socket_world(s, Some("vfx_top")).position, self.socket_world(s, Some("vfx_bottom")).position));
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
        self.live.iter().filter(|l| matches!(l.anchor, Anchor::Sora(..)) && !l.stopped).filter_map(|l| Some((l.id, self.anchor_xform(l, s)?))).collect()
    }

    /// (INT, measuring aid) A socket / bone of his (or of the weapon) in the world for scene `s` (`vfx_top`: the blade's tip).
    pub fn socket_point(&self, s: &Scene, name: &str) -> Vec3 {
        self.socket_world(s, Some(name)).position
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
}

impl EffectHost {
    /// (VFXR) The latest draw list's character drives, owner resolved (`draw` first).
    pub fn char_drives(&self) -> Vec<CharDrive> {
        let sora_root = self.int_before.as_ref().map_or(Vec3::ZERO, |(o, _)| o.position);
        self.list
            .char_params
            .iter()
            .map(|p| {
                let owner = self.live.iter().find(|l| l.id.0 == p.effect).map_or(0, |l| if let Anchor::Party(m, ..) = &l.anchor { m.saturating_add(1) } else { 0 });
                let mut value = p.value;
                if p.name == "InLightCenterPosition" {
                    let root = if owner == 0 { sora_root } else { self.party_members.get(&(owner - 1)).map_or(sora_root, |m| m.position) };
                    value = kh_vfx::screen::in_light_center(Vec3::new(value[0], value[1], value[2]), root);
                }
                CharDrive { owner, name: p.name.clone(), value, vector: p.vector, target: p.target }
            })
            .collect()
    }
}
