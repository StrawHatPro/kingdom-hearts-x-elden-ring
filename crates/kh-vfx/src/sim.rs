//! The evaluator: effect instances, their emitter nodes and particles, stepped at the host's
//! fixed rate and drawn between two steps (`World::draw(alpha, ..)`).
//!
//! One step of an emitter instance, in the order of the game's driver 0x1442A1380 [C] and of
//! stock `FParticleEmitterInstance::Tick` [U]:
//! 1. emitter time and timeline (start, intro, loop, ending; docs/sora-vfx-spec.md 5);
//! 2. the node: `SQEX_EmitterInitializeTransforms` once, the `SQEX_EmitterTransforms` tracks,
//!    then the node matrix = base (component or parent node, per `SQEX_EmitterTakeover` /
//!    `SQEX_ComponentCutoff`) x local (0x144290A60 [C] for the local part);
//! 3. `SQEX_Parent`: child emitter instances at their generate times;
//! 4. particles: age and kill, reset to base values, update modules;
//! 5. spawning (rate and bursts) with the spawn modules;
//! 6. integration (`Location += Velocity * dt`), `SQEX_AllParticleKill`.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use glam::{Affine3A, Mat3, Vec2, Vec3};

use crate::def::*;
use crate::draw::*;
use crate::math::*;
use crate::rng::Rng;

/// One spawned effect. Never reused.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EffectId(pub u64);

/// How an effect is started.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpawnOptions {
    /// The seed of its random stream; `None` = from the world's spawn counter.
    pub seed: Option<u32>,
    /// `ParticleColor` / `ParticleAlpha` of the attach row: multiplies everything.
    pub color: [f32; 4],
    /// `StartDelayTime`: seconds before anything ticks.
    pub start_delay: f32,
    /// `FadeIn`: seconds from invisible to full.
    pub fade_in: f32,
}

impl Default for SpawnOptions {
    fn default() -> SpawnOptions {
        SpawnOptions { seed: None, color: [1.0; 4], start_delay: 0.0, fade_in: 0.0 }
    }
}

#[derive(Clone, Debug, Default)]
struct Particle {
    loc: Vec3,
    prev_loc: Vec3,
    vel: Vec3,
    base_vel: Vec3,
    /// 0..1 over its life.
    rel: f32,
    inv_life: f32,
    /// Seconds since spawn.
    age: f32,
    size: Vec3,
    base_size: Vec3,
    color: [f32; 4],
    base_color: [f32; 4],
    /// Radians.
    rot: f32,
    rot_rate: f32,
    base_rot_rate: f32,
    /// Euler degrees (stock mesh rotation).
    mesh_rot: Vec3,
    mesh_rot_rate: Vec3,
    /// `SQEX_ParticleRotation`: start, accumulated, the value sampled at spawn, and the result.
    prot_start: Vec3,
    prot_acc: Vec3,
    prot_sample: Vec3,
    prot_added: Vec3,
    has_prot: bool,
    sub: f32,
    /// (ZA) `SubUVMovie`: the image the movie starts on (`StartingFrame`).
    sub_start: f32,
    dynamic: [f32; 4],
    cam_offset: f32,
    depth_offset: f32,
    orbit: Vec3,
    /// The node's rotation when it was spawned (mesh particles of world-space emitters).
    spawn_rot: Mat3,
    attach_rot: bool,
    /// `spawn_rot` is the rotation its spawn shape gave it (in the simulation space).
    shape_rot: bool,
    /// Ring corner it sits on (`bAttachParticle`), or `u32::MAX`; its fraction along the edge and its picks between the two radii.
    ring_vertex: u32,
    ring_fraction: f32,
    ring_mix: [f32; 2],
    /// `SQEX_LocalParticle`: the particle's place in the node's frame.
    is_local: bool,
    local_rot: bool,
    local: Vec3,
    /// `SQEX_ParticleScale`: the start value.
    pscale_start: Vec3,
    just_spawned: bool,
    /// The second edge of an AnimTrail sample.
    edge: Vec3,
    /// Sub meshes: the random start phase, 0..1.
    mesh_phase: f32,
    /// (XW-d) `age` at the previous step (drawing between steps) and a fragment mesh particle's own random stream seed
    /// (its pieces' impact draws; 0 for every other kind, whose random stream is untouched).
    prev_age: f32,
    seed: u32,
    /// (VFXR) Collision: hits so far and the most it takes (sampled at spawn); frozen by a completion; hit this step;
    /// to be removed (a kill); SQEX_SphereForce's size scale applied; SQEX_ParticleLocation's current offset.
    hits: u32,
    max_hits: f32,
    frozen: bool,
    collided: bool,
    dead: bool,
    hit_scaled: bool,
    ploc_offset: Vec3,
}

/// What a child node reads from its parent (the block at emitter instance+0x1E0 [C]).
#[derive(Clone, Copy, Debug)]
struct NodeData {
    /// The component as this node sees it (+0x1F0) and its scale (+0x230).
    comp: Rigid,
    comp_scale: Vec3,
    /// The node's own matrix with its parents, without the component and without scale (+0x2D0).
    local: Rigid,
    /// The node scale down the chain (+0x390) and the particle size multiplier (+0x4AC).
    chain_scale: Vec3,
    psize: Vec3,
    /// `bLeavePlaceInWorld` (+0x1E0): children leave their place too.
    leave: bool,
}

impl Default for NodeData {
    fn default() -> NodeData {
        NodeData { comp: Rigid::IDENTITY, comp_scale: Vec3::ONE, local: Rigid::IDENTITY, chain_scale: Vec3::ONE, psize: Vec3::ONE, leave: false }
    }
}

#[derive(Clone, Debug, Default)]
struct ChildState {
    fired: Vec<bool>,
    count: u32,
}

#[derive(Clone, Debug)]
struct Emitter {
    def: usize,
    parent: Option<usize>,
    /// Seconds since the instance was created.
    time: f32,
    /// Emitter time: seconds since `EmitterStartTime` (what distributions are sampled with).
    rel: f32,
    started: bool,
    first_tick: bool,
    loop_count: u32,
    /// The timeline has ended: nothing spawns any more.
    ended: bool,
    done: bool,
    loop_end: bool,
    spawn_fraction: f32,
    bursts_fired: Vec<bool>,
    children: Vec<ChildState>,
    // ---- node
    acc_pos: Vec3,
    acc_rot: Vec3,
    acc_scale: Vec3,
    track_pos: Vec3,
    track_rot: Vec3,
    track_scale: Vec3,
    track_vel: Vec3,
    travel: Vec3,
    init_pos: Vec3,
    init_rot: Vec3,
    init_scale: Vec3,
    gravity: f32,
    gravity_velocity: f32,
    gravity_offset: f32,
    /// The child offset given by the parent (`ChildrenData` + its spawn shape).
    offset_pos: Vec3,
    offset_rot: Vec3,
    offset_scale: Vec3,
    offset_velocity: Vec3,
    /// The component (rotation + translation, and its scale) and the parent's block on the first tick.
    comp_snapshot: (Rigid, Vec3),
    parent_snapshot: Option<NodeData>,
    /// What children read (the game's block at instance+0x1E0).
    data: NodeData,
    /// The node in the world, with its world scale on the axes.
    node_prev: Affine3A,
    node: Affine3A,
    /// Simulation space -> world (identity for a world-space emitter).
    sim_prev: Affine3A,
    sim: Affine3A,
    node_sim_prev: Rigid,
    /// The component scale as cut by `SQEX_ComponentCutoff` (instance+0x230).
    comp_scale: Vec3,
    /// The particle size multiplier (instance+0x4B8).
    psize_final: Vec3,
    noise_offset: Vec3,
    ring: RingState,
    /// A child its parent generates anew in every loop: it plays its timeline once.
    one_shot: bool,
    /// `SQEX_SpawnParticleSystem`: asked for / done.
    nested_wanted: bool,
    nested_spawned: bool,
    /// `SQEX_CameraShake`: played (once per instance), and what is waiting for the host.
    shake_fired: bool,
    shake_pending: Option<ShakeRequest>,
    /// `SQEX_Raycast`: where the ray put the node, relative to where it was (None = not cast yet).
    ray_shift: Option<Vec3>,
    /// A tail's recorded node positions (world), oldest first.
    path: Vec<Vec3>,
    path_prev_head: Vec3,
    pivot: Vec2,
    mesh_pivot: Vec3,
    particles: Vec<Particle>,
    /// (VFXR) the events its particles sent this step (kind, world place, world velocity); the places other emitters'
    /// events asked it to spawn at (world place, world velocity, inherit); the spawn-per-unit carry; the tail wave's random
    /// vector and its clock.
    events_out: Vec<(u8, Vec3, Vec3)>,
    pending: Vec<(Vec3, Vec3)>,
    unit_fraction: f32,
    wave_vec: Vec3,
    wave_clock: f32,
}

impl Emitter {
    fn new(def: usize, d: &EmitterDef, parent: Option<usize>) -> Emitter {
        let mut pivot = Vec2::ZERO;
        let mut mesh_pivot = Vec3::ZERO;
        for m in &d.modules {
            match m {
                Module::PivotOffset(p) => pivot = *p,
                Module::MeshPivotOffset(p) => mesh_pivot = *p,
                _ => {}
            }
        }
        Emitter {
            def,
            parent,
            time: 0.0,
            rel: 0.0,
            started: false,
            first_tick: true,
            loop_count: 0,
            ended: false,
            done: false,
            loop_end: false,
            spawn_fraction: 0.0,
            bursts_fired: vec![false; d.bursts.len()],
            children: d.children.iter().map(|c| ChildState { fired: vec![false; c.times.len()], count: 0 }).collect(),
            acc_pos: Vec3::ZERO,
            acc_rot: Vec3::ZERO,
            acc_scale: Vec3::ONE,
            track_pos: Vec3::ZERO,
            track_rot: Vec3::ZERO,
            track_scale: Vec3::ONE,
            track_vel: Vec3::ZERO,
            travel: Vec3::ZERO,
            init_pos: Vec3::ZERO,
            init_rot: Vec3::ZERO,
            init_scale: Vec3::ONE,
            gravity: 0.0,
            gravity_velocity: 0.0,
            gravity_offset: 0.0,
            offset_pos: Vec3::ZERO,
            offset_rot: Vec3::ZERO,
            offset_scale: Vec3::ONE,
            offset_velocity: Vec3::ZERO,
            comp_snapshot: (Rigid::IDENTITY, Vec3::ONE),
            parent_snapshot: None,
            data: NodeData::default(),
            node_prev: Affine3A::IDENTITY,
            node: Affine3A::IDENTITY,
            sim_prev: Affine3A::IDENTITY,
            sim: Affine3A::IDENTITY,
            node_sim_prev: Rigid::IDENTITY,
            comp_scale: Vec3::ONE,
            psize_final: Vec3::ONE,
            noise_offset: Vec3::ZERO,
            ring: RingState::default(),
            one_shot: false,
            nested_wanted: false,
            nested_spawned: false,
            shake_fired: false,
            shake_pending: None,
            ray_shift: None,
            path: Vec::new(),
            path_prev_head: Vec3::ZERO,
            pivot,
            mesh_pivot,
            particles: Vec::new(),
            events_out: Vec::new(), // (VFXR)
            pending: Vec::new(),
            unit_fraction: 0.0,
            wave_vec: Vec3::ZERO,
            wave_clock: 0.0,
        }
    }
}

struct System {
    id: EffectId,
    def: Arc<EffectDef>,
    rng: Rng,
    xform_prev: Xform,
    xform: Xform,
    emitters: Vec<Emitter>,
    loop_end: bool,
    /// The component fade value (`FadeIn` / `FadeOut`, 0x1442633A0 / 0x144262FF0 [I]).
    fade: f32,
    fade_rate: f32,
    color: [f32; 4],
    delay: f32,
    /// The two sockets of an AnimTrail, this step and the one before.
    trail: Option<(Vec3, Vec3)>,
    trail_on: bool,
    dead: bool,
    steps: u32,
    /// Seconds stepped since the spawn (the start delay included).
    seconds: f32,
    /// A system an emitter of another one spawned (`SQEX_SpawnParticleSystem`): whom it follows.
    follow: Option<Follow>,
    /// (IMPL_ER53) A held system (`EffectDef::hold`) was stopped: its clock runs again and it plays out as authored.
    released: bool,
    /// (DON-MAGIC tint) the recolour of everything it draws: (r, g, b, intensity), see `World::set_tint`.
    recolor: Option<[f32; 4]>,
    /// (ZETTA) the bones (world place and rotation, by name) of what it hangs on: `SQEX_BoneAttach` emitters (World::set_bones)
    bones: Option<Arc<HashMap<String, (Vec3, glam::Quat)>>>,
    /// (IMPL_ER56) Its particle lights light characters too (`Light::characters`; a host's choice, default true).
    char_lights: bool,
    /// (INT) Where the host says its component was at the step BEFORE it was spawned (an attached effect born on a moving
    /// socket): its first step's emitters are drawn between that place and the spawn's, as the socket they hang on is.
    first_prev: Option<Xform>,
    /// (INT) The component the latest step started from (None: it stood still): between the steps the emitters are drawn
    /// on the component blended as a RIGID transform (place lerped, rotation slerped, as the bone it hangs on is drawn)
    /// with their place in it blended, not by a straight line between the two steps' world places (a fast swing's chord
    /// cut inside the arc the drawn blade makes).
    draw_from: Option<Xform>,
    /// (INT) This frame's component AS THE HOST DRAWS WHAT IT HANGS ON (his drawn socket) and, for an AnimTrail, the drawn
    /// sockets of its head: set just before the draw, cleared after it (World::set_drawn / clear_drawn).
    drawn_component: Option<Xform>,
    drawn_trail: Option<(Vec3, Vec3)>,
}

#[derive(Clone, Debug)]
struct Follow {
    parent: EffectId,
    /// Index of the spawning emitter instance in the parent's list.
    emitter: usize,
    def: NestedDef,
}

/// A camera shake an effect asks the host for (`SQEX_CameraShake`). The host scales it by the
/// camera's distance from `position`: 1 at `distance[1]` cm or nearer, `distance[2]` at
/// `distance[0]` cm or farther (only when `distance[2]` < 1).
#[derive(Clone, Debug, PartialEq)]
pub struct ShakeRequest {
    pub class: String,
    pub scale: f32,
    pub duration: Option<f32>,
    pub position: Vec3,
    pub distance: [f32; 3],
}

/// The effects of one host.
pub struct World {
    defs: HashMap<String, Arc<EffectDef>>,
    systems: Vec<System>,
    next_id: u64,
    materials: Vec<String>,
    material_ids: HashMap<String, u32>,
    meshes: Vec<String>,
    mesh_ids: HashMap<String, u32>,
    log: Vec<String>,
    logged: HashSet<String>,
    /// Ids of the materials / meshes of each (effect, emitter), filled at registration.
    ids: HashMap<String, Vec<(u32, u32, u32)>>,
    /// The level's ground height (cm) for `SQEX_Raycast`; None = no ground known.
    ground: Option<f32>,
    /// Mesh ids of each emitter's sub meshes, per effect.
    sub_ids: HashMap<String, Vec<Vec<u32>>>,
    shakes: Vec<ShakeRequest>,
    /// (INT) A discontinuity (a teleport, a reseat, a warp) since the last step: everything is drawn at the latest step
    /// until the next one (no blend across the jump). Cleared by `step`.
    draw_snap: bool,
}

impl Default for World {
    fn default() -> World {
        World::new()
    }
}

/// Hard limits that keep a mis-authored or misread effect from eating the frame.
const MAX_EMITTERS_PER_SYSTEM: usize = 256;
const MAX_SPAWN_PER_STEP: usize = 200;
const MAX_SYSTEMS: usize = 400;

const NO_ID: u32 = u32::MAX;

impl World {
    pub fn new() -> World {
        World { defs: HashMap::new(), systems: Vec::new(), next_id: 1, materials: Vec::new(), material_ids: HashMap::new(), meshes: Vec::new(), mesh_ids: HashMap::new(), log: Vec::new(), logged: HashSet::new(), ids: HashMap::new(), ground: None, sub_ids: HashMap::new(), shakes: Vec::new(), draw_snap: false /* (INT) */ }
    }

    fn material_id(&mut self, name: &str) -> u32 {
        if name.is_empty() {
            return NO_ID;
        }
        if let Some(id) = self.material_ids.get(name) {
            return *id;
        }
        let id = self.materials.len() as u32;
        self.materials.push(name.to_string());
        self.material_ids.insert(name.to_string(), id);
        id
    }

    fn mesh_id(&mut self, name: &str) -> u32 {
        if let Some(id) = self.mesh_ids.get(name) {
            return *id;
        }
        let id = self.meshes.len() as u32;
        self.meshes.push(name.to_string());
        self.mesh_ids.insert(name.to_string(), id);
        id
    }

    /// The id a draw item of the host's own (a mesh actor that is no particle system) uses for a
    /// material / mesh of the pack: the same ids the effects' draw items carry.
    pub fn intern_material(&mut self, name: &str) -> u32 {
        self.material_id(name)
    }

    pub fn intern_mesh(&mut self, name: &str) -> u32 {
        self.mesh_id(name)
    }

    /// The material name (`materials/<name>.json` of the pack) of a draw item's `material`.
    pub fn material_name(&self, id: u32) -> Option<&str> {
        self.materials.get(id as usize).map(String::as_str)
    }

    pub fn mesh_name(&self, id: u32) -> Option<&str> {
        self.meshes.get(id as usize).map(String::as_str)
    }

    /// Makes an effect spawnable under its own name. Everything it has that the evaluator
    /// skips is logged once, naming effect, emitter and module.
    pub fn register(&mut self, def: EffectDef) {
        for (emitter, module) in def.unknown() {
            let line = format!("vfx: {}: emitter {}: {} is not evaluated (skipped)", def.name, emitter, module);
            if self.logged.insert(line.clone()) {
                self.log.push(line);
            }
        }
        let ids = def
            .emitters
            .iter()
            .map(|e| {
                let material = self.material_id(&e.material);
                let mesh = e.mesh.as_deref().map_or(NO_ID, |m| self.mesh_id(m));
                let decal = e.decal.as_ref().map_or(NO_ID, |d| self.material_id(&d.material));
                (material, mesh, decal)
            })
            .collect();
        self.ids.insert(def.name.clone(), ids);
        let sub: Vec<Vec<u32>> = def.emitters.iter().map(|e| e.sub_meshes.as_ref().map_or(Vec::new(), |s| s.list.iter().map(|m| self.mesh_id(m)).collect())).collect();
        self.sub_ids.insert(def.name.clone(), sub);
        self.defs.insert(def.name.clone(), Arc::new(def));
    }

    pub fn has(&self, name: &str) -> bool {
        self.defs.contains_key(name)
    }

    pub fn def(&self, name: &str) -> Option<&EffectDef> {
        self.defs.get(name).map(|d| &**d)
    }

    /// The camera shakes the effects asked for since the last call.
    pub fn take_shakes(&mut self) -> Vec<ShakeRequest> {
        std::mem::take(&mut self.shakes)
    }

    /// Lines logged since the last call (skipped modules, refused spawns).
    pub fn take_log(&mut self) -> Vec<String> {
        std::mem::take(&mut self.log)
    }

    /// Starts an effect at `xform`. `None` when no effect of that name is registered (logged
    /// once per name).
    pub fn spawn(&mut self, name: &str, xform: Xform, options: SpawnOptions) -> Option<EffectId> {
        let Some(def) = self.defs.get(name).cloned() else {
            let line = format!("vfx: no effect named {name}: nothing is drawn for it");
            if self.logged.insert(line.clone()) {
                self.log.push(line);
            }
            return None;
        };
        let id = EffectId(self.next_id);
        self.next_id += 1;
        let emitters = def.emitters.iter().enumerate().filter(|(_, e)| e.enabled && e.auto_add).map(|(i, e)| Emitter::new(i, e, None)).collect();
        let fading_in = options.fade_in > 0.0;
        self.systems.push(System {
            id,
            def,
            rng: Rng::new(options.seed.unwrap_or(id.0 as u32).wrapping_mul(0x9E37_79B1)),
            xform_prev: xform,
            xform,
            emitters,
            loop_end: false,
            fade: if fading_in { 0.0 } else { 1.0 },
            fade_rate: if fading_in { 1.0 / options.fade_in } else { 0.0 },
            color: options.color,
            delay: options.start_delay.max(0.0),
            trail: None,
            trail_on: false,
            dead: false,
            steps: 0,
            seconds: 0.0,
            follow: None,
            released: false,
            recolor: None, // (DON-MAGIC tint)
            bones: None, // (ZETTA)
            char_lights: true,
            first_prev: None,
            draw_from: None,
            drawn_component: None,
            drawn_trail: None,
        });
        Some(id)
    }

    fn system(&mut self, id: EffectId) -> Option<&mut System> {
        self.systems.iter_mut().find(|s| s.id == id)
    }

    /// Where the effect's component is now (call before `step` for an attached effect).
    /// (ZETTA) The bones (world place and rotation, by name) of the skeleton the effect hangs on, for its `SQEX_BoneAttach`
    /// emitters (a host gives them every step for an effect on a character; None: none).
    pub fn set_bones(&mut self, id: EffectId, bones: Option<Arc<HashMap<String, (Vec3, glam::Quat)>>>) {
        if let Some(s) = self.system(id) {
            s.bones = bones;
        }
    }

    /// True when the effect has an emitter whose particles sit on bones (`SQEX_BoneAttach`): its host feeds `set_bones`.
    pub fn wants_bones(&self, id: EffectId) -> bool {
        self.systems.iter().find(|s| s.id == id).is_some_and(|s| s.def.emitters.iter().any(|e| e.bone_attach.is_some()))
    }

    pub fn set_transform(&mut self, id: EffectId, xform: Xform) {
        if let Some(s) = self.system(id) {
            s.xform = xform;
        }
    }

    /// (IMPL_ER19) The effect's own colour multiplier from now on (what `SpawnOptions::color` set at its start:
    /// `ParticleColor` / `ParticleAlpha`, multiplies everything it draws).
    pub fn set_color(&mut self, id: EffectId, color: [f32; 4]) {
        if let Some(s) = self.system(id) {
            s.color = color;
        }
    }

    /// (DON-MAGIC tint) A per-spawn RECOLOUR (kh_core::Tint; the plan's [R] marks): everything the effect and the nested
    /// systems it spawns draw from now on gets rgb = max(r, g, b) x (r, g, b of `tint`) x its intensity (`tint[3]`), alpha
    /// unchanged. None: its own colours again.
    pub fn set_tint(&mut self, id: EffectId, tint: Option<[f32; 4]>) {
        if let Some(s) = self.system(id) {
            s.recolor = tint;
        }
    }

    /// The two sockets of an AnimTrail effect now (the game's `BeginTrails(first, second,
    /// FromSecond, 1.0)`): one ribbon sample is taken per step while they are given.
    pub fn set_trail(&mut self, id: EffectId, first: Vec3, second: Vec3) {
        if let Some(s) = self.system(id) {
            s.trail = Some((first, second));
            s.trail_on = true;
        }
    }

    /// `immediate`: destroyed now. Otherwise the game's `EnableLoopEnd`: looping emitters stop
    /// looping, play their ending and run out.
    pub fn stop(&mut self, id: EffectId, immediate: bool) {
        if let Some(s) = self.system(id) {
            if immediate {
                s.dead = true;
            } else {
                s.loop_end = true;
                s.trail_on = false;
                s.released = true;
            }
        }
    }

    /// (LDI, the user 2026-10-22: things "load instantly when the game loads") THE EFFECT AS IF IT HAD BEEN RUNNING FOR
    /// `seconds`: this one system is stepped alone, at the world's fixed step (1/60 s), right after its spawn, so a looping
    /// effect (a save point's idle base and body) is first drawn in its steady state instead of filling up from nothing.
    /// Its camera shakes are dropped; a nested system it asks for is made at the next `step` as usual. At most 10 s.
    pub fn prewarm(&mut self, id: EffectId, seconds: f32) {
        const DT: f32 = 1.0 / 60.0;
        let ground = self.ground;
        let Some(s) = self.systems.iter_mut().find(|s| s.id == id && !s.dead) else { return };
        let steps = (seconds.clamp(0.0, 10.0) / DT).round() as usize;
        for _ in 0..steps {
            if s.dead {
                break;
            }
            step_system(s, DT, ground);
            for e in s.emitters.iter_mut() {
                e.shake_pending = None;
            }
        }
        // (INT) nothing to blend from: its first drawn frame is the warmed state
        s.xform_prev = s.xform;
        s.first_prev = None;
    }

    /// The game's `FadeOut(t)`: the component fades linearly to 0 over `seconds`, then loop
    /// end; `seconds <= 0` ends at once.
    pub fn fade_out(&mut self, id: EffectId, seconds: f32) {
        if let Some(s) = self.system(id) {
            s.trail_on = false;
            s.released = true;
            if seconds <= 0.0 {
                s.dead = true;
            } else {
                s.fade_rate = -s.fade / seconds;
            }
        }
    }

    /// (IMPL_ER56) Whether this effect's particle lights light characters too (they always light the world).
    pub fn set_lights_on_characters(&mut self, id: EffectId, on: bool) {
        if let Some(s) = self.system(id) {
            s.char_lights = on;
        }
    }

    /// (IMPL_ER75) The history of the effect's AnimTrail ribbons is dropped (a discontinuity: a teleport, a jump of the
    /// world under him): the next samples start a new ribbon at the blade.
    pub fn clear_trail(&mut self, id: EffectId) {
        if let Some(s) = self.systems.iter_mut().find(|s| s.id == id) {
            let def = s.def.clone();
            for e in s.emitters.iter_mut().filter(|e| def.emitters.get(e.def).is_some_and(|d| d.kind == crate::def::Kind::AnimTrail)) {
                e.particles.clear();
            }
        }
    }

    /// (INT) Where the host's component of the effect was at the step before its spawn (an effect born attached to a
    /// moving socket: its first step is drawn between that place and the spawn's). Call right after `spawn`.
    pub fn set_previous_transform(&mut self, id: EffectId, xform: Xform) {
        if let Some(s) = self.system(id) {
            if s.steps == 0 {
                s.first_prev = Some(xform);
            }
        }
    }

    /// (INT) THE HOST'S DRAWN PLACE of what an effect hangs on, for the draw that follows (his drawn socket: the component;
    /// the trail's two drawn sockets): its emitters are drawn on it, between their places in it at the two steps, and the
    /// trail's head at the drawn blade. Then clear_drawn after the draw.
    pub fn set_drawn(&mut self, id: EffectId, component: Option<Xform>, trail: Option<(Vec3, Vec3)>) {
        if let Some(s) = self.system(id) {
            (s.drawn_component, s.drawn_trail) = (component.filter(|c| c.position.is_finite()), trail.filter(|t| t.0.is_finite() && t.1.is_finite()));
        }
    }

    /// (INT) Every drawn place a host gave is forgotten.
    pub fn clear_drawn(&mut self) {
        for s in &mut self.systems {
            (s.drawn_component, s.drawn_trail) = (None, None);
        }
    }

    /// (INT) A DISCONTINUITY (a teleport, a reseat, a warp of what the effects hang on): until the next step everything
    /// is drawn at the latest step, never between the place before the jump and the one after it.
    pub fn snap_draw(&mut self) {
        self.draw_snap = true;
    }

    /// (INT, measuring aid) The effect's emitter nodes as `draw` places them `alpha` of the way from the step before to
    /// the latest one (the same blend as the draw), one entry per emitter instance (None: it draws nothing now).
    pub fn drawn_nodes(&self, id: EffectId, alpha: f32) -> Vec<Option<Vec3>> {
        let alpha = if self.draw_snap { 1.0 } else { alpha };
        let Some(s) = self.systems.iter().find(|s| s.id == id && !s.dead && s.delay <= 0.0) else { return Vec::new() };
        let comp_blend = comp_blend(s, alpha);
        let blend = |prev: &Affine3A, now: &Affine3A| match comp_blend {
            Some((c, x0, x1)) => c * lerp_affine(&(x0 * *prev), &(x1 * *now), alpha),
            None => lerp_affine(prev, now, alpha),
        };
        s.emitters.iter().map(|e| (e.started && !e.done).then(|| Vec3::from(blend(&e.node_prev, &e.node).translation))).collect()
    }

    /// (INT, measuring aid) The effect's component as of the latest step and its emitter nodes then (the order of
    /// `drawn_nodes`).
    pub fn latest_nodes(&self, id: EffectId) -> Option<(Xform, Vec<Option<Vec3>>)> {
        let s = self.systems.iter().find(|s| s.id == id && !s.dead && s.delay <= 0.0)?;
        Some((s.xform, s.emitters.iter().map(|e| (e.started && !e.done).then(|| Vec3::from(e.node.translation))).collect()))
    }

    /// (INT, measuring aid) The drawn head of the effect's AnimTrail ribbon at `alpha` (what `draw` puts first).
    pub fn drawn_trail_head(&self, id: EffectId, alpha: f32) -> Option<Vec3> {
        let alpha = if self.draw_snap { 1.0 } else { alpha };
        let s = self.systems.iter().find(|s| s.id == id && !s.dead)?;
        s.emitters.iter().filter(|e| s.def.emitters.get(e.def).is_some_and(|d| d.kind == crate::def::Kind::AnimTrail)).filter(|e| e.particles.len() >= 2).find_map(|e| trail_head(s, &e.particles, alpha).map(|(a, _)| a)) // (a ribbon of one sample is not drawn)
    }

    /// (IMPL_ER75) The newest sample of the effect's AnimTrail ribbon (its first socket) and how many samples it holds.
    pub fn trail_head(&self, id: EffectId) -> Option<(Vec3, usize)> {
        let s = self.systems.iter().find(|s| s.id == id && !s.dead)?;
        s.emitters.iter().filter(|e| s.def.emitters.get(e.def).is_some_and(|d| d.kind == crate::def::Kind::AnimTrail)).find_map(|e| e.particles.last().map(|p| (p.loc, e.particles.len())))
    }

    pub fn is_alive(&self, id: EffectId) -> bool {
        self.systems.iter().any(|s| s.id == id && !s.dead)
    }

    /// (effects, emitter instances, particles) alive.
    pub fn stats(&self) -> (usize, usize, usize) {
        (self.systems.len(), self.systems.iter().map(|s| s.emitters.len()).sum(), self.systems.iter().map(|s| s.emitters.iter().map(|e| e.particles.len()).sum::<usize>()).sum())
    }

    /// The height of the ground under the effects (cm), for the emitters that cast a ray to find
    /// it (`SQEX_Raycast`: Thunder's ground marks). A host with real collision answers per ray;
    /// the test level's floor is flat.
    pub fn set_ground(&mut self, z: Option<f32>) {
        self.ground = z;
    }

    /// One fixed step of every effect.
    pub fn step(&mut self, dt: f32) {
        self.draw_snap = false; // (INT) the step after a discontinuity: drawn between the steps again
        // Nested systems take the node of the emitter that spawned them (0x1442FE100 [C]).
        for k in 0..self.systems.len() {
            let Some(follow) = self.systems[k].follow.clone() else { continue };
            let parent = self.systems.iter().find(|p| p.id == follow.parent && !p.dead).and_then(|p| p.emitters.get(follow.emitter).map(|e| (e.node, e.loop_end || e.done, e.rel, p.color, p.recolor)));
            let child = &mut self.systems[k];
            match parent {
                Some((node, ending, time, color, recolor)) => {
                    // (DON-MAGIC tint) a nested system is recoloured as its spawner
                    if recolor.is_some() {
                        child.recolor = recolor;
                    }
                    let scale = scale_of(&node);
                    if follow.def.give_location {
                        child.xform.position = Vec3::from(node.translation);
                    }
                    if follow.def.give_rotation {
                        child.xform.rotation = Mat3::from_cols(Vec3::from(node.matrix3.x_axis).normalize_or(Vec3::X), Vec3::from(node.matrix3.y_axis).normalize_or(Vec3::Y), Vec3::from(node.matrix3.z_axis).normalize_or(Vec3::Z));
                    }
                    if follow.def.give_scale {
                        child.xform.scale = scale;
                    }
                    if follow.def.give_color {
                        child.color[..3].copy_from_slice(&color[..3]);
                    }
                    if follow.def.give_alpha {
                        child.color[3] = color[3];
                    }
                    if (follow.def.loop_end_time > 0.0 && time >= follow.def.loop_end_time) || (follow.def.loop_end_time == 0.0 && ending) {
                        child.loop_end = true;
                    }
                }
                // The spawner is gone: the nested system runs out.
                None => child.loop_end = true,
            }
        }
        let ground = self.ground;
        let mut requests: Vec<(EffectId, usize, NestedDef, Xform, [f32; 4])> = Vec::new();
        // The systems do not touch one another while they step (each has its own random stream, so
        // the result is the same whatever runs them): with many particles alive they are stepped
        // on scoped threads, in even shares of the particles.
        let load: usize = self.systems.iter().map(|s| s.emitters.iter().map(|e| e.particles.len()).sum::<usize>()).sum();
        let threads = if load < 4000 || self.systems.len() < 2 { 1 } else { std::thread::available_parallelism().map_or(1, |n| n.get()).min(8) };
        if threads <= 1 {
            self.systems.iter_mut().filter(|s| !s.dead).for_each(|s| step_system(s, dt, ground));
        } else {
            let share = load / threads + 1;
            let mut parts: Vec<&mut [System]> = Vec::new();
            let mut remaining = self.systems.as_mut_slice();
            while !remaining.is_empty() {
                let (mut count, mut held) = (0, 0);
                while count < remaining.len() && (held < share || count == 0) {
                    held += remaining[count].emitters.iter().map(|e| e.particles.len()).sum::<usize>();
                    count += 1;
                }
                let (head, tail) = remaining.split_at_mut(count);
                parts.push(head);
                remaining = tail;
            }
            std::thread::scope(|scope| {
                for part in parts {
                    scope.spawn(move || part.iter_mut().filter(|s| !s.dead).for_each(|s| step_system(s, dt, ground)));
                }
            });
        }
        for s in &mut self.systems {
            if !s.dead {
                for (index, e) in s.emitters.iter_mut().enumerate() {
                    if let Some(shake) = e.shake_pending.take() {
                        self.shakes.push(shake);
                    }
                    if e.nested_wanted && !e.nested_spawned {
                        e.nested_spawned = true;
                        if let Some(n) = &s.def.emitters[e.def].nested {
                            let node = e.node;
                            let rotation = Mat3::from_cols(Vec3::from(node.matrix3.x_axis).normalize_or(Vec3::X), Vec3::from(node.matrix3.y_axis).normalize_or(Vec3::Y), Vec3::from(node.matrix3.z_axis).normalize_or(Vec3::Z));
                            requests.push((s.id, index, n.clone(), Xform { position: Vec3::from(node.translation), rotation, scale: scale_of(&node) }, s.color));
                        }
                    }
                }
            }
        }
        for (parent, emitter, def, xform, color) in requests {
            if self.systems.len() >= MAX_SYSTEMS {
                break;
            }
            let options = SpawnOptions { color, ..SpawnOptions::default() };
            let asset = def.asset.clone();
            // (IMPL_ER56) a nested system lights characters as its spawner does
            let char_lights = self.systems.iter().find(|p| p.id == parent).is_none_or(|p| p.char_lights);
            if let Some(id) = self.spawn(&asset, xform, options) {
                if let Some(child) = self.system(id) {
                    child.char_lights = char_lights;
                    child.follow = Some(Follow { parent, emitter, def });
                }
            }
        }
        self.systems.retain(|s| !s.dead);
    }

    /// Everything to draw, `alpha` of the way from the previous step to the latest one.
    pub fn draw(&self, alpha: f32, camera: &Camera, out: &mut DrawList) {
        let alpha = if self.draw_snap { 1.0 } else { alpha }; // (INT) no blend across a discontinuity
        for s in &self.systems {
            if s.dead || s.delay > 0.0 {
                continue;
            }
            let Some(ids) = self.ids.get(&s.def.name) else { continue };
            let empty = Vec::new();
            let sub = self.sub_ids.get(&s.def.name).unwrap_or(&empty);
            // (DON-MAGIC tint) what this system adds to the list is recoloured
            let marks = (out.quads.len(), out.ribbons.len(), out.meshes.len(), out.lights.len(), out.decals.len());
            draw_system(s, ids, sub, alpha, camera, out);
            if let Some(t) = s.recolor {
                recolor_from(out, marks, t);
            }
            screen_of(s, alpha, camera, out); // (VFXR)
        }
    }
}

/// (VFXR) The screen-wide passes and the character-material drives of one system's live emitters, at their emitter time
/// (the node blended between the steps as it is drawn). A pass is faded with the component (FadeIn / FadeOut) and the
/// attach row's alpha.
fn screen_of(s: &System, alpha: f32, camera: &Camera, out: &mut DrawList) {
    let fade = s.fade.clamp(0.0, 1.0) * s.color[3].clamp(0.0, 1.0);
    for e in s.emitters.iter().filter(|e| e.started && !e.done) {
        let d = &s.def.emitters[e.def];
        if d.screen.is_empty() && d.char_params.is_empty() {
            continue;
        }
        let node = Vec3::from(e.node_prev.translation).lerp(Vec3::from(e.node.translation), alpha);
        for sd in &d.screen {
            if let Some(mut fx) = sd.sample(e.rel, node, fade) {
                if fx.fade_distance > 0.0 {
                    fx.alpha *= (1.0 - node.distance(camera.position) / fx.fade_distance).clamp(0.0, 1.0);
                }
                if fx.alpha.abs() > 1.0e-5 {
                    out.screen.push(fx);
                }
            }
        }
        for cd in &d.char_params {
            cd.sample(s.id.0, e.rel, node, camera.position, &mut out.char_params);
        }
    }
}

/// (DON-MAGIC tint) The recolour of one colour: its brightest channel times the tint and its intensity, alpha kept.
pub fn recolor(c: [f32; 4], t: [f32; 4]) -> [f32; 4] {
    let v = c[0].max(c[1]).max(c[2]);
    [v * t[0] * t[3], v * t[1] * t[3], v * t[2] * t[3], c[3]]
}

/// (DON-MAGIC tint) Everything added to `out` since `marks` is TINTED: the ranges are recorded for the renderer, which
/// recolours the shader's final colour (a vertex colour may be data to the material: it is left as it is). Only the
/// particle lights (no shader) take the recolour here.
fn recolor_from(out: &mut DrawList, marks: (usize, usize, usize, usize, usize), t: [f32; 4]) {
    for (kind, from, to) in [(0u8, marks.0, out.quads.len()), (1, marks.1, out.ribbons.len()), (2, marks.2, out.meshes.len()), (3, marks.4, out.decals.len())] {
        if to > from {
            out.tints.push(crate::draw::TintRange { kind, from: from as u32, to: to as u32, tint: t });
        }
    }
    for l in &mut out.lights[marks.3..] {
        let c = recolor([l.color.x, l.color.y, l.color.z, 1.0], t);
        l.color = Vec3::new(c[0], c[1], c[2]);
    }
}

// ------------------------------------------------------------------------------------- stepping

fn step_system(s: &mut System, dt: f32, ground: Option<f32>) {
    // (IMPL_ER53) a held system's clock stops at its hold time: it is still stepped (its node follows its component) with
    // no time passing
    let dt = match s.def.hold.filter(|_| !s.released) {
        Some(hold) if s.seconds >= hold => 0.0,
        Some(hold) => dt.min(hold - s.seconds).max(0.0),
        None => dt,
    };
    s.seconds += dt;
    if s.delay > 0.0 {
        s.delay -= dt;
        s.xform_prev = s.xform;
        return;
    }
    // (INT) the component's move over this step as one rigid motion: an emitter's FIRST tick (the system's first step, or
    // an emitter born later in a moving system) takes its previous node from it, so that between the two steps it is drawn
    // where its component was, as the socket it hangs on is (before: at the latest step's place, up to a step ahead of him)
    let component_prev = if s.steps == 0 { s.first_prev.unwrap_or(s.xform_prev) } else { s.xform_prev };
    let component_delta = (component_prev != s.xform).then(|| component_prev.affine() * s.xform.affine().inverse()).filter(|d| d.is_finite());
    s.draw_from = component_delta.map(|_| component_prev);
    // Component fade (TickComponent 0x144270D30 [C]).
    if s.fade_rate != 0.0 {
        s.fade += s.fade_rate * dt;
        if s.fade_rate < 0.0 && s.fade <= 0.0 {
            // Faded out: loop end, and nothing of it is visible any more.
            s.dead = true;
            return;
        }
        if s.fade >= 1.0 {
            s.fade = 1.0;
            s.fade_rate = 0.0;
        }
    }
    let def = s.def.clone();
    let mut i = 0;
    while i < s.emitters.len() {
        if s.emitters[i].done {
            i += 1;
            continue;
        }
        let parent = s.emitters[i].parent.map(|p| (s.emitters[p].data, s.emitters[p].loop_end || s.emitters[p].done));
        // An emitter with live children keeps its node moving for them.
        let children_alive = s.emitters.iter().any(|c| c.parent == Some(i) && !c.done);
        let di = s.emitters[i].def;
        let mut emitter = std::mem::replace(&mut s.emitters[i], Emitter::new(0, &def.emitters[di], None));
        let mut born = Vec::new();
        let ctx = Tick { dt, def: &def, d: &def.emitters[emitter.def], component: s.xform, system_loop_end: s.loop_end, parent, trail: if s.trail_on { s.trail } else { None }, index: i, children_alive, ground, component_delta, bones: s.bones.clone() /* (ZETTA) */ };
        tick_emitter(&mut emitter, &ctx, &mut s.rng, &mut born);
        s.emitters[i] = emitter;
        if s.emitters.len() + born.len() <= MAX_EMITTERS_PER_SYSTEM {
            s.emitters.append(&mut born);
        }
        i += 1;
    }
    // (VFXR) EventGenerator -> EventReceiverSpawn: every event of this step goes to the emitters that receive its name
    if def.emitters.iter().any(|d| !d.receivers.is_empty()) {
        let mut deliveries: Vec<(usize, Vec3, Vec3)> = Vec::new();
        for src in s.emitters.iter().filter(|e| !e.events_out.is_empty()) {
            let gen = &def.emitters[src.def];
            for (kind, at, vel) in &src.events_out {
                for name in gen.events.iter().filter(|ev| ev.kind == *kind).map(|ev| &ev.name) {
                    for (k, dst) in s.emitters.iter().enumerate() {
                        let dd = &def.emitters[dst.def];
                        if dst.done || !dd.enabled {
                            continue;
                        }
                        for r in dd.receivers.iter().filter(|r| &r.name == name && (r.kind == 255 || r.kind == *kind)) {
                            let n = r.count.float(0.0, &mut s.rng).round().max(0.0) as usize;
                            let v = r.inherit_velocity.as_ref().map_or(Vec3::ZERO, |sc| *vel * sc.vec3(0.0, &mut s.rng));
                            for _ in 0..n.min(16) {
                                deliveries.push((k, *at, v));
                            }
                        }
                    }
                }
            }
        }
        for (k, at, v) in deliveries {
            if s.emitters[k].pending.len() < 256 {
                s.emitters[k].pending.push((at, v));
            }
        }
        // (an emitter that is done is not ticked again: its last events are delivered once)
        for e in &mut s.emitters {
            e.events_out.clear();
        }
    }
    // (VFXR) SQEX_EmitterDirectEX: the node stands where the named emitter's is [?: by the property's name]
    for k in 0..s.emitters.len() {
        let Some(name) = def.emitters[s.emitters[k].def].direct_ex.as_ref() else { continue };
        if let Some(at) = s.emitters.iter().find(|o| !o.done && &def.emitters[o.def].name == name).map(|o| o.node.translation) {
            s.emitters[k].node.translation = at;
        }
    }
    s.xform_prev = s.xform;
    s.steps += 1;
    if s.emitters.iter().all(|e| e.done) {
        s.dead = true;
    }
}

struct Tick<'a> {
    dt: f32,
    def: &'a EffectDef,
    d: &'a EmitterDef,
    component: Xform,
    system_loop_end: bool,
    /// The parent's node block and whether it is ending.
    parent: Option<(NodeData, bool)>,
    trail: Option<(Vec3, Vec3)>,
    /// (INT) The component's rigid move from the step before to this one (None: it stood still).
    component_delta: Option<Affine3A>,
    index: usize,
    children_alive: bool,
    ground: Option<f32>,
    /// (ZETTA) the system's bones (SQEX_BoneAttach)
    bones: Option<Arc<HashMap<String, (Vec3, glam::Quat)>>>,
}

fn wrap_degrees(v: f32) -> f32 {
    // 0x144294B90 [C]: v - trunc(v / 360) * 360, then + 360 when negative.
    let w = v - (v / 360.0).trunc() * 360.0;
    if w < 0.0 {
        w + 360.0
    } else {
        w
    }
}

fn unit_axes(m: &Affine3A) -> Mat3 {
    Mat3::from_cols(Vec3::from(m.matrix3.x_axis).normalize_or(Vec3::X), Vec3::from(m.matrix3.y_axis).normalize_or(Vec3::Y), Vec3::from(m.matrix3.z_axis).normalize_or(Vec3::Z))
}

fn tick_emitter(e: &mut Emitter, cx: &Tick, rng: &mut Rng, born: &mut Vec<Emitter>) {
    let d = cx.d;
    let dt = cx.dt;
    e.node_prev = e.node;
    e.sim_prev = e.sim;
    e.time += dt;
    if cx.system_loop_end || cx.parent.is_some_and(|(_, ending)| ending) {
        e.loop_end = true;
    }
    if e.time < d.start_time {
        if e.loop_end {
            // Asked to end before it began.
            e.done = true;
        }
        return;
    }
    // ---- 1. emitter time (getter 0x144284A30 [C]: raw time - EmitterStartTime, not below 0)
    if !e.started {
        e.started = true;
        e.rel = 0.0;
    } else {
        e.rel += dt;
    }
    let may_loop = !e.loop_end && !e.one_shot && (d.loops == 0 || e.loop_count + 1 < d.loops);
    if may_loop && !e.ended && e.rel >= d.loop_duration {
        let length = d.loop_duration - d.loop_time;
        if length > 1.0e-4 {
            e.rel = d.loop_time + (e.rel - d.loop_duration) % length;
            e.loop_count += 1;
            e.bursts_fired.iter_mut().zip(&d.bursts).for_each(|(fired, b)| {
                if b.time * d.duration >= d.loop_time {
                    *fired = false;
                }
            });
            for (state, child) in e.children.iter_mut().zip(&d.children) {
                for (fired, (time, _)) in state.fired.iter_mut().zip(&child.times) {
                    if *time >= d.loop_time && (child.loop_number == 0 || e.loop_count < child.loop_number) {
                        *fired = false;
                    }
                }
            }
        } else {
            // A loop of no length: the emitter holds at the loop point until loop end.
            e.rel = d.loop_duration;
        }
    }
    let end_time = d.duration_time.max(d.loop_duration);
    // An AnimTrail runs for as long as the host feeds its sockets (the notify window).
    let fed = d.kind == Kind::AnimTrail && cx.trail.is_some();
    if !e.ended && !fed && e.rel > end_time && !(may_loop && d.loop_duration - d.loop_time <= 1.0e-4) {
        // `bKillOnCompleted` is not applied here: in stock Cascade it takes effect when the whole
        // system completes [U], and 3,231 of 3,241 emitters set it while their particles outlive
        // the emitter's own timeline (a 0.1 s burst emitter with 0.5 s particles).
        e.ended = true;
    }
    let t = e.rel;

    // ---- 2. the node. Order of the driver 0x1442A1380 [C]: tracks (0x144294B90), the component
    // part (vt+0x250 = 0x1442A0CA0), colour, scales (vt+0x398 = 0x1442A3E80), then position and
    // rotation with the parent (vt+0x248 = 0x144290A60).
    if e.first_tick {
        if let Some(init) = &d.init {
            e.init_pos = init.position.as_ref().map_or(Vec3::ZERO, |x| x.vec3(t, rng));
            e.init_rot = init.rotation.as_ref().map_or(Vec3::ZERO, |x| x.vec3(t, rng));
            e.init_scale = init.scale.as_ref().map_or(Vec3::ONE, |x| x.vec3(t, rng));
        }
        // 0x144289180 [C]: three randoms x 100, the noise offset of this emitter instance.
        e.noise_offset = Vec3::new(rng.f32(), rng.f32(), rng.f32()) * 100.0;
    }
    if let Some(tr) = &d.transforms {
        let first = e.first_tick;
        let sample = |track: &Option<Track>, last: &mut Vec3, rng: &mut Rng| -> Option<bool> {
            let track = track.as_ref()?;
            if !track.init_only || first {
                *last = track.dist.vec3(t, rng);
            }
            Some(track.direct)
        };
        match sample(&tr.position, &mut e.track_pos, rng) {
            Some(true) => e.acc_pos = e.track_pos,
            Some(false) => e.acc_pos += e.track_pos * dt,
            None => {}
        }
        match sample(&tr.rotation, &mut e.track_rot, rng) {
            Some(true) => e.acc_rot = e.track_rot,
            Some(false) => {
                let r = e.acc_rot + e.track_rot * dt;
                e.acc_rot = Vec3::new(wrap_degrees(r.x), wrap_degrees(r.y), wrap_degrees(r.z));
            }
            None => {}
        }
        match sample(&tr.scale, &mut e.track_scale, rng) {
            Some(true) => e.acc_scale = e.track_scale,
            Some(false) => e.acc_scale += e.track_scale * dt,
            None => {}
        }
        // `Velocity`: its integrator was not traced; its sum (instance+0x474) is added to the
        // position in 0x144290A60 [C].
        if sample(&tr.velocity, &mut e.track_vel, rng).is_some() {
            e.travel += e.track_vel * dt;
        }
        // Gravity (0x1442A17A0 [C]): g = dist(t) * 60; v += g * dt; z += v * dt. A uniform
        // distribution is sampled once.
        if let Some(g) = &tr.gravity {
            if e.first_tick || !g.is_random() {
                e.gravity = g.float(t, rng) * 60.0;
            }
            e.gravity_velocity += e.gravity * dt;
            e.gravity_offset += e.gravity_velocity * dt;
        }
    }
    e.travel += e.offset_velocity * dt;
    let inh = &d.inherit;
    let parent_live = cx.parent.map(|(p, _)| p);
    // -- the component part (0x1442A0CA0 [C]): rotation and translation only, scale kept apart.
    let live = Rigid { rot: cx.component.rotation, pos: cx.component.position };
    if e.first_tick {
        // A child starts from what its parent holds; a root from the component as it is now.
        e.comp_snapshot = parent_live.map_or((live, cx.component.scale), |p| (p.comp, p.comp_scale));
        e.parent_snapshot = parent_live;
    }
    let leave = parent_live.is_some_and(|p| p.leave) || inh.leave_place;
    let mut comp = live;
    if leave {
        comp = e.comp_snapshot.0;
    } else if inh.cutoff_rotation == Some(Takeover::SpawnTimeOnly) {
        comp.rot = e.comp_snapshot.0.rot;
    }
    let comp_scale = match inh.cutoff_scale {
        Some(Takeover::None) => Vec3::ONE,
        _ if leave => e.comp_snapshot.1,
        Some(Takeover::SpawnTimeOnly) => e.comp_snapshot.1,
        _ => cx.component.scale,
    };
    // -- scales (0x1442A3E80 [C])
    let pick_parent = |mode: Takeover| match mode {
        Takeover::Always => parent_live,
        Takeover::SpawnTimeOnly => e.parent_snapshot,
        Takeover::None => None,
    };
    let own_scale = e.init_scale * e.acc_scale;
    let parent_scale = if inh.takeover { pick_parent(inh.parent_scale).map_or(Vec3::ONE, |p| p.chain_scale) } else { Vec3::ONE };
    let parent_psize = if inh.takeover { pick_parent(inh.parent_particle_size).map_or(Vec3::ONE, |p| p.psize) } else { Vec3::ONE };
    // 0x144294B90 [C]: the SizeScale module's value at emitter time when it runs on emitter time.
    let emitter_size = d.size_scale_emitter.as_ref().map_or(Vec3::ONE, |s| s.vec3(t, rng));
    let base_scale = cx.def.base_scale;
    let chain_scale = own_scale * parent_scale * e.offset_scale;
    let psize = own_scale * parent_psize * e.offset_scale * emitter_size;
    e.psize_final = if inh.takeover && !inh.particle_size { Vec3::ONE } else { own_scale } * parent_psize * e.offset_scale * emitter_size * base_scale;
    let world_scale = if d.local_space { Vec3::ONE } else { comp_scale } * chain_scale * base_scale;
    // -- position and rotation (0x144290A60 [C]): position = track + init + EmitterOrigin + the
    // child offset, times the node's world scale with `bIsUseTransformScale`; rotation = the sum of
    // the Euler angles of the same four sources in `RotationPriority` order.
    let mut own_pos = e.acc_pos + e.init_pos + d.origin + e.offset_pos;
    if d.transforms.as_ref().is_some_and(|tr| tr.use_transform_scale) {
        own_pos *= world_scale;
        if d.emitter_local {
            own_pos *= comp_scale;
        }
    }
    let travel_scale = if d.local_space && !d.emitter_local { Vec3::ONE } else { comp_scale };
    let travel_parent = if inh.takeover && inh.parent_scale != Takeover::None { parent_live.map_or(Vec3::ONE, |p| p.chain_scale) } else { Vec3::ONE };
    own_pos += travel_scale * e.travel * travel_parent;
    let mut local = Rigid { rot: d.priority.matrix(e.acc_rot + e.init_rot + d.rotation + e.offset_rot), pos: own_pos };
    if parent_live.is_some() {
        // The parent's matrix is its own local one (instance+0x2D0): no scale, no component.
        let (position, rotation) = if inh.takeover { (pick_parent(inh.parent_position), pick_parent(inh.parent_rotation)) } else { (parent_live, parent_live) };
        if let Some(p) = position {
            local.pos = p.local.point(local.pos);
        }
        if let Some(p) = rotation {
            local.rot = p.local.rot * local.rot;
        }
    }
    // -- to the simulation space: the world for a world-space emitter, the component's space with
    // `bUseLocalSpace` (the node is then `EmitterToSimulation`).
    let cut_rotation = inh.cutoff_rotation == Some(Takeover::None);
    let mut node_sim = local;
    if !d.local_space {
        node_sim = if cut_rotation { Rigid { rot: local.rot, pos: local.pos + comp.pos } } else { comp.then(&local) };
        if inh.takeover {
            match inh.component_position {
                Takeover::None => node_sim.pos = comp.pos + own_pos,
                Takeover::SpawnTimeOnly => node_sim.pos = comp.pos + e.comp_snapshot.0.rot * own_pos,
                Takeover::Always => {}
            }
        }
    }
    // Gravity moves the finished matrix along Z (0x1442A17A0 is called on it).
    node_sim.pos.z += e.gravity_offset * if d.local_space { 1.0 } else { comp_scale.z };
    // -- simulation -> world, and the node in the world with its scale (what decals, tails, nested
    // systems and lights read).
    let comp_rot = if cut_rotation { Mat3::IDENTITY } else { comp.rot };
    let (sim, node_world, node_world_scale) = if !d.local_space {
        (Affine3A::IDENTITY, node_sim, world_scale)
    } else if d.emitter_local {
        // 0x144284A80 [C]: local x scale(node world scale) x scale(component) x component.
        let s = world_scale * comp_scale;
        let m = Affine3A::from_mat3_translation(comp_rot * Mat3::from_diagonal(s) * local.rot, comp.pos + comp_rot * (s * local.pos));
        (m, Rigid { rot: comp_rot * local.rot, pos: Vec3::from(m.translation) }, s)
    } else {
        let m = Affine3A::from_mat3_translation(comp.rot * Mat3::from_diagonal(comp_scale), comp.pos);
        (m, Rigid { rot: comp.rot * local.rot, pos: m.transform_point3(local.pos) }, world_scale * comp_scale)
    };
    e.sim = sim;
    e.node = Affine3A::from_mat3_translation(node_world.rot * Mat3::from_diagonal(node_world_scale), node_world.pos);
    e.comp_scale = comp_scale;
    e.data = NodeData { comp, comp_scale, local, chain_scale, psize, leave };
    // SQEX_Raycast [?]: once, the node is moved to where its ray meets the ground.
    if let Some(rc) = &d.raycast {
        if e.ray_shift.is_none() {
            let start = Vec3::from(e.node.translation);
            let dir = if rc.world { rc.end_offset } else { node_world.rot * rc.end_offset };
            let hit = cx.ground.and_then(|z| {
                let (a, b) = (start.z - z, start.z + dir.z - z);
                (dir.z.abs() > 1.0e-6 && a * b <= 0.0).then(|| start + dir * ((z - start.z) / dir.z))
            });
            match hit {
                Some(point) => e.ray_shift = Some(point + rc.offset - start),
                None if rc.force => e.ray_shift = Some(Vec3::ZERO),
                None => {
                    // ESQEX_RAYCAST_SPAWN_THEME_ONHIT and nothing hit: the emitter does not play.
                    e.done = true;
                    e.first_tick = false;
                    return;
                }
            }
        }
        if let Some(shift) = e.ray_shift {
            e.node.translation += glam::Vec3A::from(shift);
            if !d.local_space {
                node_sim.pos += shift;
            }
        }
    }
    if let Some(n) = &d.nested {
        if !e.nested_spawned && e.rel > n.spawn_time {
            e.nested_wanted = true;
        }
    }
    if let Some(shake) = &d.camera_shake {
        // 0x1442FC5E0 [C]: once per (module, instance), from ShakePlayTime on.
        if !e.shake_fired && e.rel >= shake.play_time {
            e.shake_fired = true;
            e.shake_pending = Some(ShakeRequest { class: shake.class.clone(), scale: shake.scale.float(e.rel, rng), duration: shake.duration, position: Vec3::from(e.node.translation), distance: shake.distance });
        }
    }
    if e.first_tick {
        e.node_prev = e.node;
        e.sim_prev = e.sim;
        e.node_sim_prev = node_sim;
        // (INT) born on a moving component: its place at the step before is the component's move applied backwards
        if let Some(d) = cx.component_delta {
            e.node_prev = d * e.node;
            e.sim_prev = d * e.sim;
        }
    }
    // (INT) the node in the simulation space at the step before (the newborn particles' draw rule below)
    let node_sim_before = e.node_sim_prev.pos;
    // The shape speed's scale (0x14435B130 [C]) and the Velocity module's (0x144335AD0 [C]).
    let plain_space = !d.local_space && !d.emitter_local;
    let speed_scale = if plain_space { cx.component.scale * base_scale } else if cx.def.base_scale_bugfix { base_scale } else { Vec3::ONE };
    let space = Space {
        origin: if d.emitter_local { Vec3::ZERO } else { node_sim.pos },
        rot: if d.emitter_local { Mat3::IDENTITY } else { node_sim.rot },
        world_scale: if d.emitter_local { Vec3::ONE } else { world_scale },
        speed_scale,
        velocity_scale: if d.emitter_local { Vec3::ONE } else { world_scale },
        world_to_sim: if d.local_space { unit_axes(&sim).transpose() } else { Mat3::IDENTITY },
        local_particles: d.modules.iter().any(|m| matches!(m, Module::LocalParticle { position: true, .. })),
        psize: e.psize_final,
        noise_offset: e.noise_offset,
        seconds: e.time,
        mesh: d.kind == Kind::Mesh || d.kind == Kind::Fragment,
        ground: cx.ground, // (VFXR)
        to_world: sim,
        bones: cx.bones.clone(),
    };
    e.node_sim_prev = node_sim;

    // ---- 3. children (SQEX_Parent::Update 0x14437A3B0 [C])
    if !e.ended {
        for (k, child) in d.children.iter().enumerate() {
            for (n, (time, number)) in child.times.iter().enumerate() {
                let state = &mut e.children[k];
                // (XW-e) LoopNumber counts the loops of the parent's LOOPED section [loop_time, loop_duration): an entry timed
                // there generates in its first LoopNumber passes. An entry timed in the END section (at or past loop_duration:
                // reached once, after the parent's loop has ended) generates once when the end section reaches it, whatever
                // the loop count: the pack's 36 such entries are its "end" children (pg_loopend -> c_kira_loopend, cps3_end_gen
                // -> c3_end_smk0 / c3_end_dst, Control_lp -> end_par / end_mdl*, zmn0 Control_lp -> ps_br ...), which the
                // count gate never let through once the parent had looped (xwe/tools/scan_children.js). [D] by the data.
                let looped = *time >= d.loop_time && *time < d.loop_duration;
                if state.fired[n] || t < *time || (looped && child.loop_number != 0 && e.loop_count >= child.loop_number) {
                    continue;
                }
                state.fired[n] = true;
                // (fourth pass) A ring of order ALL places one child on EVERY vertex per generation,
                // as it does particles (Water's cast ring: three ribbons 120 degrees apart; with
                // one the ring was a single short arc). [D] by the data and the capture.
                let per = match (child.use_spawn_module, d.shape().map(|s| &s.kind)) {
                    (true, Some(ShapeKind::Ring { vertices, order: RingOrder::All, .. })) => (vertices.float(t, rng).round().max(1.0)) as u32,
                    _ => 1,
                };
                for _ in 0..*number * per {
                    if child.max_count != 0 && state.count >= child.max_count {
                        break;
                    }
                    state.count += 1;
                    let cd = &cx.def.emitters[child.emitter];
                    // (IMPL_ER53) a host's copy leaves out the children it disabled
                    if cx.def.variant && !cd.enabled {
                        break;
                    }
                    let mut c = Emitter::new(child.emitter, cd, Some(cx.index));
                    c.offset_pos = child.offset_position;
                    c.offset_rot = child.offset_rotation;
                    c.offset_scale = child.offset_scale;
                    // Not traced [?]: whether a child loops. Read from the data: a child generated in
                    // every loop of its parent (LoopNumber 0) plays once and is replaced by the next
                    // generation; one generated once loops with its own EmitterLoops.
                    c.one_shot = child.loop_number == 0;
                    if child.use_spawn_module {
                        if let Some(shape) = d.shape() {
                            // Slot 114 of the shape module [C]: its point, scaled, in the node's frame
                            // (the child's own chain applies the parent's rotation).
                            let s = sample_shape(shape, t, 0.0, rng, &mut e.ring, Vec3::ONE, None);
                            c.offset_pos += s.point;
                            // The shape's Speed moves the nodes that are generated again and again
                            // (radiating streaks); a node that stays would drift away for ever [?].
                            if c.one_shot {
                                c.offset_velocity = s.direction * s.speed;
                            }
                        }
                    }
                    born.push(c);
                }
            }
        }
    }

    // ---- 4. particles: age, kill, reset, update
    // (VFXR) the events this emitter sends (EventGenerator): its particles' deaths, collisions and births, in the world
    let sends = |kind: u8| d.events.iter().any(|ev| ev.kind == kind);
    let (send_death, send_hit, send_birth) = (sends(1), sends(2), sends(0));
    e.events_out.clear();
    let to_world = e.sim;
    let events_out = &mut e.events_out;
    e.particles.retain_mut(|p| {
        p.prev_loc = p.loc;
        p.prev_age = p.age;
        p.rel += p.inv_life * dt;
        p.age += dt;
        if p.rel >= 1.0 && send_death {
            events_out.push((1, to_world.transform_point3(p.loc), to_world.transform_vector3(p.vel)));
        }
        p.rel < 1.0
    });
    for p in &mut e.particles {
        p.vel = p.base_vel;
        p.size = p.base_size;
        p.color = p.base_color;
        p.rot_rate = p.base_rot_rate;
        p.collided = false; // (VFXR)
        for m in &d.modules {
            update_module(m, p, t, dt, &space, rng);
        }
    }
    // (VFXR) collisions sent, killed particles gone (a kill is a death too)
    if e.particles.iter().any(|p| p.collided || p.dead) {
        let events_out = &mut e.events_out;
        e.particles.retain(|p| {
            if p.collided && send_hit {
                events_out.push((2, to_world.transform_point3(p.loc), to_world.transform_vector3(p.vel)));
            }
            if p.dead && send_death {
                events_out.push((1, to_world.transform_point3(p.loc), to_world.transform_vector3(p.vel)));
            }
            !p.dead
        });
    }

    // ---- 5. spawning
    let first_new = e.particles.len(); // (INT)
    // (VFXR) EventReceiverSpawn: the places other emitters' events asked for (handed over by step_system)
    if !e.pending.is_empty() && d.kind != Kind::Control && d.kind != Kind::Unsupported {
        let to_sim = e.sim.inverse();
        for (at, vel) in std::mem::take(&mut e.pending) {
            if e.particles.len() >= d.max_particles {
                break;
            }
            let mut p = new_particle(d, &space, t, 0.0, rng, &mut e.ring);
            let local = Vec3::from(to_sim.transform_point3a(at.into()));
            p.loc = p.loc - space.origin + local;
            p.prev_loc = p.loc;
            let v = to_sim.transform_vector3(vel);
            p.vel += v;
            p.base_vel += v;
            e.particles.push(p);
        }
    }
    if !e.ended && e.particles.len() < d.max_particles {
        let mut count = 0usize;
        if d.kind == Kind::AnimTrail {
            // One ribbon sample per step at the two sockets (stock AnimTrail spawns per tick).
            if let Some((a, b)) = cx.trail {
                let mut p = new_particle(d, &space, t, 0.0, rng, &mut e.ring);
                p.loc = a;
                p.prev_loc = a;
                p.edge = b;
                p.vel = Vec3::ZERO;
                p.base_vel = Vec3::ZERO;
                e.particles.push(p);
            }
        } else if d.kind == Kind::Tail || d.kind == Kind::TailSprites {
            // A tail emitter spawns nothing: the tail is the node's own track, and its size and
            // colour are the emitter's (one carrier particle that lives as long as the emitter).
            if e.particles.is_empty() {
                let mut p = new_particle(d, &space, t, 0.0, rng, &mut e.ring);
                p.inv_life = 0.0;
                p.just_spawned = false;
                e.particles.push(p);
            }
        } else if d.kind != Kind::Control && d.kind != Kind::Unsupported {
            let rate = d.rate.float(t, rng) * d.rate_scale.float(t, rng);
            if rate > 0.0 {
                e.spawn_fraction += rate * dt;
                count = e.spawn_fraction as usize;
                e.spawn_fraction -= count as f32;
            }
            // (VFXR) SpawnPerUnit: particles per unit of the node's travel this step (UE 4.17 [U]: the travel below
            // MovementTolerance spawns none; with bIgnoreSpawnRateWhenMoving the rate is dropped while it moves)
            let mut unit_count = 0usize;
            if let Some(su) = &d.spawn_per_unit {
                let mut moved = Vec3::from(e.node.translation - e.node_prev.translation);
                for a in 0..3 {
                    if !su.axes[a] {
                        moved[a] = 0.0;
                    }
                }
                let distance = moved.length();
                let distance = if su.max_frame_distance > 0.0 && distance > su.max_frame_distance { 0.0 } else { distance };
                if distance > su.tolerance && !e.first_tick {
                    e.unit_fraction += distance / su.unit * su.per_unit.float(t, rng);
                    unit_count = e.unit_fraction.max(0.0) as usize;
                    e.unit_fraction -= unit_count as f32;
                    if su.ignore_rate_when_moving {
                        count = 0;
                    }
                }
            }
            let rate_count = count;
            let mut burst_count = 0usize;
            for (fired, b) in e.bursts_fired.iter_mut().zip(&d.bursts) {
                if !*fired && t >= b.time * d.duration {
                    *fired = true;
                    let n = if b.count_low >= 0 && b.count_low < b.count { b.count_low + (rng.f32() * (b.count - b.count_low + 1) as f32) as i32 } else { b.count };
                    burst_count += (n.max(0) as f32 * d.burst_scale.float(t, rng)).round().max(0.0) as usize;
                }
            }
            // `EVARTEX_ODER_TYPE_ALL`: one particle on every ring vertex per spawn [?] (the spawn
            // count's multiplication was not traced).
            let per = match d.shape().map(|s| &s.kind) {
                Some(ShapeKind::Ring { vertices, order: RingOrder::All, .. }) => (vertices.float(t, rng).round().max(1.0)) as usize,
                _ => 1,
            };
            let total = ((rate_count + burst_count) * per).min(MAX_SPAWN_PER_STEP.max(d.max_particles / 4)).min(d.max_particles - e.particles.len());
            for k in 0..total {
                // Rate particles are spread over the step; burst particles are born at its start.
                let fraction = if k < rate_count * per && rate_count > 0 { (k / per) as f32 / rate_count as f32 } else { 0.0 };
                let p = new_particle(d, &space, t, dt * (1.0 - fraction).clamp(0.0, 1.0) * if k < rate_count * per { 1.0 } else { 0.0 }, rng, &mut e.ring);
                e.particles.push(p);
            }
            // (VFXR) the spawn-per-unit particles, spread along the node's travel of the step
            let from = if d.local_space { node_sim_before } else { Vec3::from(e.node_prev.translation) };
            let unit_total = unit_count.min(MAX_SPAWN_PER_STEP).min(d.max_particles.saturating_sub(e.particles.len()));
            for k in 0..unit_total {
                let mut p = new_particle(d, &space, t, 0.0, rng, &mut e.ring);
                let f = (k as f32 + 0.5) / unit_total as f32;
                let at = space.origin.lerp(from, 1.0 - f);
                p.loc += at - space.origin;
                p.prev_loc = p.loc;
                e.particles.push(p);
            }
        }
    }

    // (VFXR) birth events
    if send_birth {
        for p in &e.particles[first_new..] {
            e.events_out.push((0, e.sim.transform_point3(p.loc), e.sim.transform_vector3(p.vel)));
        }
    }
    // (INT) THE NEWBORN PARTICLES ARE DRAWN WITH THEIR EMITTER between the steps: a particle born at this step's node has
    // no place at the step before, and drawn at its birth place it led the emitter (and the bone it hangs on) by up to a
    // step's move; its place "before" is its birth place moved back with the node (world space: the node's world move;
    // local space: the node's move in the component, the component's own move being in the drawn `sim`). Ribbons and
    // tails take their heads at draw time (`trail_head_at`, `path_prev_head`).
    if !matches!(d.kind, Kind::AnimTrail | Kind::Tail | Kind::TailSprites) && e.particles.len() > first_new {
        let back = if d.local_space { node_sim.pos - node_sim_before } else { Vec3::from(e.node.translation - e.node_prev.translation) };
        if back != Vec3::ZERO && back.is_finite() {
            for p in &mut e.particles[first_new..] {
                p.prev_loc = p.loc - back;
            }
        }
    }

    // ---- 6. integration
    let tail = d.kind == Kind::Tail || d.kind == Kind::TailSprites;
    if tail {
        // The tail helper records the node's transform every tick [C]; the points are set along
        // this path when drawing.
        let offset = d.tail.as_ref().map_or(Vec3::ZERO, |tl| tl.offset);
        let mut world = e.node.transform_point3(offset);
        // (VFXR) SQEX_TailShapeExData_Inertia: the recorded head lags behind the node by TailInertia (per 1/60 s) [A]
        if let (Some(inertia), Some(last)) = (&d.tail_inertia, e.path.last().copied()) {
            let k = inertia.float(t, rng).clamp(0.0, 0.99).powf(dt * 60.0);
            world = world.lerp(last, k);
        }
        // (VFXR) SQEX_TailShapeExData_Wave: every recorded point drifts by MoveVector + a random vector redrawn every
        // RandomInterval seconds (cm / s, in the node's axes with bUseEmitterMat; world axes otherwise) [A]
        if let Some(wave) = &d.tail_wave {
            e.wave_clock -= dt;
            if e.wave_clock <= 0.0 {
                e.wave_vec = wave.random.as_ref().map_or(Vec3::ZERO, |r| r.vec3(t, rng));
                e.wave_clock = wave.interval.max(dt);
            }
            let drift = unit_axes(&e.node) * (wave.move_vector + e.wave_vec) * dt;
            for q in &mut e.path {
                *q += drift;
            }
        }
        e.path_prev_head = e.path.last().copied().unwrap_or(world);
        e.path.push(world);
        // HistoryMaxDataNumber, or enough for the longest tail of the pack at any speed.
        let cap = d.tail.as_ref().map_or(0, |tl| tl.history_max).max(600);
        if e.path.len() > cap {
            let extra = e.path.len() - cap;
            e.path.drain(0..extra);
        }
        if e.ended {
            e.particles.clear();
        }
    }
    for p in &mut e.particles {
        if p.just_spawned {
            p.just_spawned = false;
        } else if p.frozen {
            // (VFXR) a Collision completion froze it where it hit
        } else {
            if p.is_local {
                // 0x14428DA00 [C]: the velocity moves the particle's place in the node's frame.
                p.local += p.vel * dt;
            } else {
                p.loc += p.vel * dt;
            }
            p.rot += p.rot_rate * dt;
            p.mesh_rot += p.mesh_rot_rate * dt;
        }
        if p.is_local {
            p.loc = space.origin + if p.local_rot { space.rot * p.local } else { p.local };
        }
        if tail {
            p.loc = space.origin;
        }
    }
    // A value that ran away (a curve authored for other conditions) must not reach the host.
    e.particles.retain(|p| p.loc.is_finite() && p.size.is_finite() && p.color.iter().all(|c| c.is_finite()));
    if let Some(kill) = d.kill_time {
        // SQEX_AllParticleKill (by its name; the reader was not located).
        if t >= kill && kill > 0.0 && !may_loop {
            e.particles.clear();
            e.ended = true;
        }
    }
    // (fourth pass) `bKillOnCompleted` for the particles that have NO lifetime of their own (no
    // Lifetime module: 1,298 of the pack's 3,500 emitters, every one with bKillOnCompleted): such
    // a particle is the emitter's own body (`SQEX_AttachParticle`, colour by emitter time) and
    // goes with the emitter's timeline. Without this it lived for ever: the Ragnarok charge star
    // on the Keyblade's tip (vw_so010_rgn_tam00 `fls1`) stayed after the shotlock. [D] by the
    // data; particles with a lifetime still run out by themselves (see the note at `ended`).
    if e.ended && d.kill_on_completed && !tail {
        e.particles.retain(|p| p.inv_life > 0.0);
    }
    e.first_tick = false;
    // (fourth pass) A scene-light curve plays to its last key (Thunder's ends 0.3 s after the
    // emitter's own timeline; cut there the light would jump back).
    if d.scene_light.as_ref().is_some_and(|(_, last)| e.rel < *last) {
        return;
    }
    // (ZETTA) SQEX_BoneAttach: particle k on socket k (modulo the list) of the host's skeleton, in the simulation space
    if let (Some(ba), Some(bones)) = (&d.bone_attach, &cx.bones) {
        if !ba.sockets.is_empty() {
            let to_sim = e.sim.inverse();
            for (k, p) in e.particles.iter_mut().enumerate() {
                if !ba.follow && p.age > cx.dt + 1.0e-6 {
                    continue;
                }
                let (name, offset) = &ba.sockets[k % ba.sockets.len()];
                let Some((pos, rot)) = bones.get(name).copied() else { continue };
                let at = Vec3::from(to_sim.transform_point3a((pos + rot * *offset).into()));
                if p.age <= cx.dt + 1.0e-6 {
                    p.prev_loc = at;
                }
                p.loc = at;
            }
        }
    }
    if e.ended && e.particles.is_empty() && !cx.children_alive {
        e.done = true;
    }
}

/// The emitter's simulation space: the world for a world-space emitter, the component's space
/// with `bUseLocalSpace`, the node's own with `bUseEmitterLocalSpace` as well.
struct Space {
    /// The node (`EmitterToSimulation`, instance+0x60): origin and rotation, no scale.
    origin: Vec3,
    rot: Mat3,
    /// The node's world scale (instance+0x39C): multiplies spawn-shape points.
    world_scale: Vec3,
    /// What a shape's Speed is multiplied by.
    speed_scale: Vec3,
    /// What the Velocity module multiplies by with `bApplyOwnerScale`.
    velocity_scale: Vec3,
    /// The rotation from the world into the simulation space.
    world_to_sim: Mat3,
    /// `SQEX_LocalParticle` with `bIsPosition`: shape points and normals stay in the node's frame.
    local_particles: bool,
    /// The emitter's particle size multiplier (instance+0x4B8).
    psize: Vec3,
    noise_offset: Vec3,
    /// Seconds since the instance was created.
    seconds: f32,
    /// A mesh emitter: spawn shapes turn the particles.
    mesh: bool,
    /// (VFXR) the ground under the effects (Collision / CollisionGPU), the simulation space -> world, the host's bones
    /// (SQEX_VertexSpawn on the attached mesh)
    ground: Option<f32>,
    to_world: Affine3A,
    bones: Option<Arc<HashMap<String, (Vec3, glam::Quat)>>>,
}

impl Space {
    /// A direction given in node space.
    fn vector(&self, v: Vec3) -> Vec3 {
        self.rot * v
    }
    fn from_world(&self, v: Vec3) -> Vec3 {
        self.world_to_sim * v
    }
}

/// The counters of `SQEX_Ring` on an emitter instance (+0x70C, +0x710).
#[derive(Clone, Copy, Debug, Default)]
struct RingState {
    step: u32,
    vertex: u32,
}

/// What a spawn shape gives: the point (scaled, in the node's frame, before the node's own
/// rotation), the direction of the initial velocity in the same frame, and the speed.
struct ShapePoint {
    point: Vec3,
    direction: Vec3,
    speed: f32,
    /// Ring: corner, fraction along the edge, and the two picks between the radii.
    ring: Option<(u32, f32, [f32; 2])>,
}

/// 0x144352710 [I]: the squared radius fraction for a `Deviation` d: with probability d it lies
/// in [d, 1], otherwise in [0, d]. d = 1 is the surface, d = 0.5 is uniform over a disc's area.
fn deviation_fraction(d: f32, rng: &mut Rng) -> f32 {
    let d = d.clamp(0.0, 1.0);
    let (r1, r2) = (rng.f32(), rng.f32());
    if 1.0 - d < r1 {
        (1.0 - d) * r2 + d
    } else {
        r2 * d
    }
}

struct RingValues {
    n: f32,
    radius: f32,
    radius_two: f32,
    interval: f32,
    angle: f32,
    position: Vec3,
    position_two: Vec3,
    rotation: Mat3,
    scale: Vec3,
}

/// Corner `k` of a ring and the next one (0x144355000 [C]).
fn ring_corners(v: &RingValues, k: u32, radius: f32, second: bool) -> (Vec3, Vec3) {
    let step = v.angle / v.n;
    let offset = v.position + if second { v.position_two } else { Vec3::ZERO };
    let at = |degrees: f32| rotator(0.0, degrees, 0.0) * Vec3::new(radius, 0.0, 0.0) + offset;
    let next = if (k as f32) < v.n * v.interval { (k + 1) as f32 * step } else { 0.0 };
    (at(k as f32 * step), at(next))
}

/// The point (corner, fraction, picks) of a ring, before the node's own rotation (0x1443484B0 [C]).
fn ring_point(v: &RingValues, direction: &Mat3, scale: Vec3, k: u32, fraction: f32, mix: [f32; 2]) -> Vec3 {
    let (mut a, mut b) = ring_corners(v, k, v.radius, false);
    if v.radius_two != 0.0 {
        let (a2, b2) = ring_corners(v, k, v.radius_two, true);
        a = a.lerp(a2, mix[0]);
        b = b.lerp(b2, mix[1]);
    }
    // vt+0x2D8 [C]: (point x scale) x (RingOffsetRotation x Direction).
    let m = *direction * v.rotation;
    let (a, b) = (m * (a * v.scale * scale), m * (b * v.scale * scale));
    a.lerp(b, fraction)
}

fn ring_values(shape: &Shape, t: f32, rng: &mut Rng) -> Option<RingValues> {
    let ShapeKind::Ring { vertices, radius, radius_two, interval, angle, offset_position, offset_position_two, offset_rotation, offset_scale, .. } = &shape.kind else { return None };
    let zero_is = |v: f32, d: f32| if v == 0.0 { d } else { v };
    Some(RingValues {
        n: zero_is(vertices.float(t, rng), 1.0),
        radius: radius.float(t, rng),
        radius_two: radius_two.as_ref().map_or(0.0, |x| x.float(t, rng)),
        interval: zero_is(interval.float(t, rng), 1.0),
        angle: zero_is(angle.as_ref().map_or(360.0, |x| x.float(t, rng)), 360.0),
        position: offset_position.as_ref().map_or(Vec3::ZERO, |x| x.vec3(t, rng)),
        position_two: offset_position_two.as_ref().map_or(Vec3::ZERO, |x| x.vec3(t, rng)),
        rotation: offset_rotation.as_ref().map_or(Mat3::IDENTITY, |x| euler(x.vec3(t, rng))),
        scale: offset_scale.as_ref().map_or(Vec3::ONE, |x| x.vec3(t, rng)),
    })
}

/// A point of a spawn shape. `scale` is the node's world scale when the shape applies it;
/// `node` the node's rotation when the velocity direction is wanted in the simulation space
/// (None: in the node's frame).
fn sample_shape(shape: &Shape, t: f32, particle_time: f32, rng: &mut Rng, ring: &mut RingState, scale: Vec3, node: Option<&Mat3>) -> ShapePoint {
    let direction = rotator(shape.direction.y, shape.direction.z, shape.direction.x);
    let scale = if shape.location_scale { scale } else { Vec3::ONE };
    let mut ring_out = None;
    // (point, the "vertex normal" of slots 117 / 112 [C])
    let (point, normal) = match &shape.kind {
        ShapeKind::Range { radius, angle, deviation, scale: range_scale } => {
            // 0x144356A00 [C]: theta = acos(1 - (1 - cos(DirectionAngle)) * r1), phi = 2 pi r2, a
            // direction inside a spherical cap about +X turned by Direction; radius =
            // sqrt(f(Deviation)) x RangeRadius.
            let r = radius.float(t, rng);
            let half = angle.as_ref().map_or(180.0, |a| a.float(t, rng)).to_radians();
            let dev = deviation.as_ref().map_or(1.0, |x| x.float(t, rng));
            let fraction = deviation_fraction(dev, rng);
            let theta = (1.0 - (1.0 - half.cos()) * rng.f32()).clamp(-1.0, 1.0).acos();
            let phi = rng.f32() * std::f32::consts::TAU;
            let dir = Vec3::new(theta.cos(), theta.sin() * phi.sin(), theta.sin() * phi.cos());
            let s = range_scale.as_ref().map_or(Vec3::ONE, |x| x.vec3(t, rng));
            // Slot 117 [C]: the vertex normal of a range is Direction's X axis.
            (direction * (dir * fraction.sqrt() * r) * s * scale, direction * Vec3::X)
        }
        ShapeKind::Ring { order, .. } => {
            let v = ring_values(shape, t, rng).expect("a ring");
            if *order == RingOrder::Random {
                // 0x14434FCB0 [C]: both counters are drawn anew.
                ring.step = ((rng.f32() * (v.interval as i32 + 1) as f32) as u32).min(v.interval.max(0.0) as u32);
                ring.vertex = ((rng.f32() * (v.n as i32 + 1) as f32) as u32).min(v.n.max(0.0) as u32);
            }
            let fraction = ring.step as f32 / v.interval;
            let mix = if v.radius_two != 0.0 { [rng.f32(), rng.f32()] } else { [0.0; 2] };
            let point = ring_point(&v, &direction, scale, ring.vertex, fraction, mix);
            ring_out = Some((ring.vertex, fraction, mix));
            ring.step += 1;
            if v.interval <= ring.step as f32 {
                ring.step = 0;
                ring.vertex += 1;
            }
            if v.n <= ring.vertex as f32 {
                ring.vertex = 0;
            }
            // Slot 112 [C] (tangent mode): the ring's axis, Z of Direction.
            (point, direction * Vec3::Z)
        }
        ShapeKind::Quad { size, scale: quad_scale, deviation } => {
            // 0x1443565C0 [C]: one face of the box is picked; on that axis the point sits at
            // +- the half extent, on the two others anywhere between.
            let face = ((rng.f32() * 3.0) as usize).min(2);
            let f = deviation_fraction(deviation.as_ref().map_or(1.0, |x| x.float(t, rng)), rng).sqrt();
            let mut local = Vec3::ZERO;
            for axis in 0..3 {
                let (sign, spread) = (rng.f32(), rng.f32());
                let mut v = f * size[axis] * if sign >= 0.5 { -1.0 } else { 1.0 };
                if axis != face {
                    v *= 2.0 * spread - 1.0;
                }
                local[axis] = v;
            }
            let s = quad_scale.as_ref().map_or(Vec3::ONE, |x| x.vec3(t, rng));
            (direction * (local * s * scale), direction * Vec3::Z)
        }
        // (VFXR) a point of the set: at random, or one after the other (ORDER / ALL)
        ShapeKind::Points { points, order, .. } => {
            if points.is_empty() {
                (Vec3::ZERO, direction * Vec3::Z)
            } else {
                let k = match order {
                    RingOrder::Random => ((rng.f32() * points.len() as f32) as usize).min(points.len() - 1),
                    _ => {
                        let k = ring.vertex as usize % points.len();
                        ring.vertex = ((k + 1) % points.len()) as u32;
                        k
                    }
                };
                let q = &points[k];
                (direction * (Vec3::new(q[0], q[1], q[2]) * scale), direction * Vec3::new(q[3], q[4], q[5]))
            }
        }
    };
    // 0x14435B130 [C]: VERTEX_NORMAL flies along the shape's normal, EMITTER_NORMAL away from the
    // emitter (the normalised point as it lies in the simulation space); NONE and VELOCITY add
    // nothing. The Speed is sampled at the particle's own time.
    let turned = |v: Vec3| node.map_or(v, |m| *m * v);
    let direction_out = match shape.dir_type {
        DirType::VertexNormal => turned(normal),
        DirType::EmitterNormal => turned(point).normalize_or_zero(),
        DirType::None | DirType::Velocity => Vec3::ZERO,
    };
    let speed = if shape.normal_direction != 0.0 { shape.speed.as_ref().map_or(0.0, |s| s.float(particle_time, rng)) * shape.normal_direction } else { 0.0 };
    ShapePoint { point, direction: direction_out, speed, ring: ring_out }
}

fn new_particle(d: &EmitterDef, space: &Space, t: f32, lead: f32, rng: &mut Rng, ring: &mut RingState) -> Particle {
    let mut p = Particle { loc: space.origin, color: [1.0; 4], base_color: [1.0; 4], spawn_rot: space.rot, ring_vertex: u32::MAX, just_spawned: true, ..Particle::default() };
    if d.sub_random {
        p.sub = (rng.f32() * (d.sub_h * d.sub_v) as f32).floor();
    }
    if d.sub_meshes.as_ref().is_some_and(|s| s.random_start) {
        p.mesh_phase = rng.f32();
    }
    if d.kind == Kind::Fragment {
        p.seed = rng.next_u32();
    }
    for m in &d.modules {
        spawn_module(m, &mut p, t, space, rng, ring);
    }
    // Post-spawn (0x14428DA00 [C]): the part of the step the particle has already lived.
    if p.is_local {
        p.local += p.vel * lead;
        p.loc = space.origin + if p.local_rot { space.rot * p.local } else { p.local };
    } else {
        p.loc += p.vel * lead;
    }
    p.rel += p.inv_life * lead;
    p.age += lead;
    p.prev_loc = p.loc;
    p.prev_age = p.age;
    p
}

fn time_of(emitter_time: bool, t: f32, p: &Particle) -> f32 {
    if emitter_time {
        t
    } else {
        p.rel
    }
}

/// (ZETTA) THE STOCK LOCATION PRIMITIVES (UE 4.17 ParticleModuleLocationPrimitiveCylinder / Sphere::SpawnEx and
/// LocationPrimitiveBase::DetermineUnitDirection): a random unit-box direction restricted to the axis halves that are on;
/// the cylinder retries (up to 50 times, then clamps) until the radial part lies in the disc, takes the height as
/// [-height / 2, height / 2] along its axis and clamps the point to the radius / half height; on the surface only, the
/// radial part (the sphere: the whole direction) is normalised. Returns (offset in the node frame, push in the node frame).
pub(crate) fn primitive(pr: &Primitive, t: f32, rng: &mut Rng) -> (Vec3, Option<Vec3>) {
    let unit = |rng: &mut Rng| {
        let mut d = Vec3::ZERO;
        for k in 0..3 {
            let r = rng.f32();
            let (pos, neg) = (pr.axes[2 * k], pr.axes[2 * k + 1]);
            d[k] = match (pos, neg) {
                (true, true) => r * 2.0 - 1.0,
                (true, false) => r,
                (false, true) => -r,
                _ => 0.0,
            };
        }
        d
    };
    let start = pr.start_location.vec3(t, rng);
    let radius = pr.radius.float(t, rng);
    if !pr.cylinder {
        let dir = unit(rng);
        let offset = if pr.surface_only { dir.normalize_or_zero() * radius } else { dir * radius };
        let vel = pr.velocity.then(|| offset * pr.velocity_scale.float(t, rng));
        return (offset + start, vel);
    }
    let half = pr.height.float(t, rng) * 0.5;
    let (r0, r1, h) = match pr.height_axis {
        0 => (1, 2, 0),
        1 => (0, 2, 1),
        _ => (0, 1, 2),
    };
    let mut dir = Vec3::ZERO;
    for attempt in 0..=50 {
        dir = unit(rng);
        let c = Vec2::new(dir[r0] * radius, dir[r1] * radius);
        if c.length_squared() <= radius * radius {
            break;
        }
        if attempt == 50 {
            let c = c.normalize_or_zero() * radius;
            dir[r0] = c.x;
            dir[r1] = c.y;
        }
    }
    let flat = Vec2::new(dir[r0], dir[r1]).normalize_or_zero();
    let mut normal = dir.normalize_or_zero();
    normal[r0] = flat.x;
    normal[r1] = flat.y;
    let mut offset = Vec3::ZERO;
    offset[h] = dir[h] * half;
    if pr.surface_only {
        offset[r0] = flat.x * radius;
        offset[r1] = flat.y * radius;
    } else {
        offset[r0] = dir[r0] * radius;
        offset[r1] = dir[r1] * radius;
    }
    let mut max = normal.abs() * radius;
    max[h] = half;
    offset = offset.clamp(-max, max);
    let vel = pr.velocity.then(|| {
        let mut v = normal;
        if pr.radial_velocity {
            v[h] = 0.0;
        }
        v * pr.velocity_scale.float(t, rng)
    });
    (offset + start, vel)
}

fn spawn_module(m: &Module, p: &mut Particle, t: f32, space: &Space, rng: &mut Rng, ring: &mut RingState) {
    match m {
        Module::Lifetime(d) => {
            let life = d.float(t, rng);
            // Several lifetime modules add up [U].
            let total = if p.inv_life > 0.0 { 1.0 / p.inv_life + life } else { life };
            p.inv_life = if total > 0.0 { 1.0 / total } else { 0.0 };
        }
        Module::Size(d) => {
            // 0x1442F5840 [C]: the start size is added as it is; no scale of the component here.
            let s = d.vec3(t, rng);
            p.size += s;
            p.base_size += s;
        }
        Module::Color { color, alpha } => {
            let c = color.vec3(t, rng);
            p.base_color = [c.x, c.y, c.z, alpha.float(t, rng)];
            p.color = p.base_color;
        }
        Module::ColorOverLife { color, alpha } => {
            let c = color.vec3(p.rel, rng);
            p.base_color = [c.x, c.y, c.z, alpha.float(p.rel, rng)];
            p.color = p.base_color;
        }
        Module::Velocity { start, radial, world, owner_scale } => {
            // 0x144335AD0 [C]: turned by the node unless given in world space; with
            // `bApplyOwnerScale` multiplied by the node's world scale, the radial part too.
            let mut v = start.vec3(t, rng);
            v = if *world { space.from_world(v) } else { space.vector(v) };
            let s = if *owner_scale { space.velocity_scale } else { Vec3::ONE };
            v *= s;
            if let Some(radial) = radial {
                v += (p.loc - space.origin).normalize_or_zero() * radial.float(t, rng) * s;
            }
            p.vel += v;
            p.base_vel += v;
        }
        Module::Rotation(d) => p.rot += d.float(t, rng) * std::f32::consts::TAU,
        Module::RotationRate(d) => {
            let r = d.float(t, rng) * std::f32::consts::TAU;
            p.rot_rate += r;
            p.base_rot_rate += r;
        }
        Module::MeshRotation(d) => p.mesh_rot += d.vec3(t, rng) * 360.0,
        Module::MeshRotationRate(d) => p.mesh_rot_rate += d.vec3(t, rng) * 360.0,
        // 0x1442F3B80 [C]: turned by the node, not scaled.
        Module::Location(d) => p.loc += space.vector(d.vec3(t, rng)),
        Module::Primitive(pr) => {
            // (ZETTA) turned by the node like Location, the push too
            let (offset, vel) = primitive(pr, t, rng);
            p.loc += space.vector(offset);
            if let Some(v) = vel {
                let v = space.vector(v);
                p.vel += v;
                p.base_vel += v;
            }
        }
        // (VFXR) SQEX_VertexSpawn on the attached mesh: the host's bones stand in for its vertices (a bone at random, a few cm
        // around it); without bones the particles start at the node as before
        Module::Shape(Shape { kind: ShapeKind::Points { attach_mesh: true, .. }, dir_type, speed, normal_direction, .. }) => {
            if let Some(bones) = space.bones.as_ref().filter(|b| !b.is_empty()) {
                let k = ((rng.f32() * bones.len() as f32) as usize).min(bones.len() - 1);
                if let Some((pos, _)) = bones.values().nth(k) {
                    let jitter = Vec3::new(rng.f32() - 0.5, rng.f32() - 0.5, rng.f32() - 0.5) * 10.0;
                    let world = *pos + jitter;
                    let sim = Vec3::from(space.to_world.inverse().transform_point3a(world.into()));
                    let out = (sim - space.origin).normalize_or_zero();
                    p.loc = sim;
                    let v = if matches!(dir_type, DirType::EmitterNormal | DirType::VertexNormal) { out * speed.as_ref().map_or(0.0, |s| s.float(p.rel, rng)) * *normal_direction } else { Vec3::ZERO };
                    p.vel += v;
                    p.base_vel += v;
                }
            }
        }
        Module::Shape(shape) => {
            // vt+0x2D8 = 0x1442881B0 [C]: the point times the node's world scale, turned by the
            // node, unless the particles live in the node's frame (`SQEX_LocalParticle`).
            let node = if space.local_particles { None } else { Some(&space.rot) };
            let s = sample_shape(shape, t, p.rel, rng, ring, space.world_scale, node);
            let offset = node.map_or(s.point, |m| *m * s.point);
            p.loc += offset;
            let v = s.direction * s.speed * if shape.owner_scale { space.speed_scale } else { Vec3::ONE };
            p.vel += v;
            p.base_vel += v;
            if space.mesh && shape.mesh_rotation && s.direction != Vec3::ZERO {
                // 0x144356E70 [C] (tangent mode): the inverse of a look-at matrix from the origin along
                // the spawn direction (a little off the axes, as the game nudges it), up = the Y axis
                // of the shape's Direction in the simulation space. The mesh's Z axis looks outward.
                let sign = if shape.normal_direction < 0.0 { -1.0 } else { 1.0 };
                let z = (s.direction * sign + Vec3::new(0.01, -0.01, 0.001)).normalize_or(Vec3::Z);
                let up = node.map_or(Mat3::IDENTITY, |m| *m) * rotator(shape.direction.y, shape.direction.z, shape.direction.x) * Vec3::Y;
                let x = up.cross(z).normalize_or_zero();
                if x != Vec3::ZERO {
                    p.spawn_rot = Mat3::from_cols(x, z.cross(x), z);
                    p.shape_rot = true;
                }
            }
            if let (Some((vertex, fraction, mix)), ShapeKind::Ring { attach: true, .. }) = (s.ring, &shape.kind) {
                p.ring_vertex = vertex;
                p.ring_fraction = fraction;
                p.ring_mix = mix;
            }
        }
        Module::Orbit { offset, .. } => p.orbit = offset.vec3(t, rng),
        Module::ParticleRotation { start, rotation, scale, .. } => {
            p.has_prot = true;
            p.prot_start = start.as_ref().map_or(Vec3::ZERO, |d| d.vec3(t, rng));
            p.prot_sample = rotation.as_ref().map_or(Vec3::ZERO, |d| d.vec3(t, rng)) * scale.as_ref().map_or(Vec3::ONE, |d| d.vec3(t, rng));
            update_module(m, p, t, 0.0, space, rng);
        }
        Module::ParticleScale { start, .. } => {
            // 0x1442F30F0 [C]: the start value is kept per particle; an axis that is 0 in the data
            // (the authored ranges give X only) follows X here, as the sprite's Y does.
            let s = start.as_ref().map_or(Vec3::ONE, |d| d.vec3(p.rel, rng));
            p.pscale_start = Vec3::new(s.x, if s.y == 0.0 { s.x } else { s.y }, if s.z == 0.0 { s.x } else { s.z });
            update_module(m, p, t, 0.0, space, rng);
        }
        Module::DepthOffset(v) => p.depth_offset = space.world_scale.x.abs() * *v,
        Module::InitSizeScale => {
            p.size *= space.psize;
            p.base_size *= space.psize;
        }
        Module::Dynamic(params) => {
            for (k, param) in params.iter().enumerate() {
                if let Some(param) = param {
                    p.dynamic[k] = param.value.float(time_of(param.emitter_time, t, p), rng);
                }
            }
        }
        Module::CameraOffset { offset, .. } => p.cam_offset = offset.float(p.rel, rng),
        Module::LocalParticle { position, local_rotation, .. } => {
            if *position {
                // 0x144373420 [C]: what the modules before it added to the location is kept as the
                // particle's place in the node's frame.
                p.is_local = true;
                p.local_rot = *local_rotation;
                p.local = p.loc - space.origin;
                p.loc = space.origin + if p.local_rot { space.rot * p.local } else { p.local };
            }
        }
        Module::AccelConst(_) | Module::Accel(_) | Module::Drag(_) | Module::PivotOffset(_) | Module::MeshPivotOffset(_) | Module::CurlNoise { .. } => {}
        // (VFXR)
        Module::Collision { max, .. } => p.max_hits = max.float(t, rng),
        Module::ParticleLocation { .. } => update_module(m, p, t, 0.0, space, rng),
        Module::SphereForce { .. } | Module::Attractor { .. } | Module::AttractorGravity { .. } | Module::CollisionGpu { .. } | Module::SizeBySpeed { .. } | Module::AccelOverLife { .. } => {}
        // (ZA) the movie's start image: StartingFrame > 1 that image, 0 a random one (UE 4.17 Spawn), else the first
        Module::SubUvMovie { start, frames, .. } => {
            let last = (*frames).max(1) - 1;
            p.sub_start = if *start > 1 { (*start as u32).min(last) as f32 } else if *start == 0 { (rng.f32() * last as f32).trunc() } else { 0.0 };
            update_module(m, p, t, 0.0, space, rng);
        }
        // Modules that do at spawn what they do every tick.
        Module::SizeScale { .. } | Module::SizeMultiplyLife { .. } | Module::ColorScaleOverLife { .. } | Module::VelocityOverLife { .. } | Module::SubUv(_) | Module::AttachParticle { .. } => update_module(m, p, t, 0.0, space, rng),
    }
}

fn update_module(m: &Module, p: &mut Particle, t: f32, dt: f32, space: &Space, rng: &mut Rng) {
    match m {
        Module::SizeScale { scale, axes, emitter_time } => {
            // 0x1442F38D0 / 0x1442FEE70 [C]: Size = BaseSize x SizeScale(particle time) x the
            // emitter's particle size multiplier; on emitter time the module's value is already in
            // that multiplier and counts as 1 here.
            let s = if *emitter_time { Vec3::ONE } else { scale.vec3(p.rel, rng) } * space.psize;
            if axes[0] {
                p.size.x = p.base_size.x * s.x;
            }
            if axes[1] {
                p.size.y = p.base_size.y * s.y;
            }
            if axes[2] {
                p.size.z = p.base_size.z * s.z;
            }
        }
        Module::SizeMultiplyLife { multiplier, axes } => {
            let s = multiplier.vec3(p.rel, rng);
            if axes[0] {
                p.size.x *= s.x;
            }
            if axes[1] {
                p.size.y *= s.y;
            }
            if axes[2] {
                p.size.z *= s.z;
            }
        }
        Module::ParticleScale { scale, scale_scale, emitter_time, .. } => {
            // 0x1442F30F0 [C] (direct form; the rate form is used by no emitter of the pack).
            let time = time_of(*emitter_time, t, p);
            let s = p.pscale_start * scale.as_ref().map_or(Vec3::ONE, |d| d.vec3(time, rng)) * scale_scale.as_ref().map_or(Vec3::ONE, |d| d.vec3(time, rng)) * space.psize;
            p.size = p.base_size * s;
        }
        Module::ColorOverLife { color, alpha } => {
            let c = color.vec3(p.rel, rng);
            p.color = [c.x, c.y, c.z, alpha.float(p.rel, rng)];
        }
        Module::ColorScaleOverLife { color, alpha, emitter_time } => {
            let time = time_of(*emitter_time, t, p);
            let c = color.vec3(time, rng);
            let a = alpha.float(time, rng);
            p.color = [p.color[0] * c.x, p.color[1] * c.y, p.color[2] * c.z, p.color[3] * a];
        }
        Module::VelocityOverLife { scale, absolute } => {
            let v = scale.vec3(p.rel, rng);
            p.vel = if *absolute { space.vector(v) } else { p.vel * v };
        }
        Module::AccelConst(a) => {
            // 0x1442CFD10 [C]: world acceleration (every emitter of the pack sets bAlwaysInWorldSpace).
            let a = space.from_world(*a) * dt;
            p.vel += a;
            p.base_vel += a;
        }
        Module::Accel(d) => {
            let a = space.vector(d.vec3(p.rel, rng)) * dt;
            p.vel += a;
            p.base_vel += a;
        }
        Module::Drag(d) => {
            let c = d.float(p.rel, rng) * dt;
            p.vel -= p.vel * c;
            p.base_vel -= p.base_vel * c;
        }
        Module::SubUv(d) => p.sub = d.float(p.rel, rng),
        // (ZA) UE 4.17 UParticleModuleSubUVMovie: the image runs at FrameRate from the start image and wraps round the grid
        Module::SubUvMovie { fps, frames, .. } => {
            let n = (*frames).max(1) as f32;
            p.sub = (p.sub_start + p.age * fps.float(p.rel, rng)).rem_euclid(n);
        }
        Module::CameraOffset { offset, spawn_only } => {
            if !spawn_only {
                p.cam_offset = offset.float(p.rel, rng);
            }
        }
        Module::Dynamic(params) => {
            for (k, param) in params.iter().enumerate() {
                if let Some(param) = param.as_ref().filter(|x| !x.spawn_only) {
                    p.dynamic[k] = param.value.float(time_of(param.emitter_time, t, p), rng);
                }
            }
        }
        Module::AttachParticle { position, rotation } => {
            if *position {
                p.loc = space.origin;
                p.local = Vec3::ZERO;
                p.prev_loc = if dt == 0.0 { p.loc } else { p.prev_loc };
            }
            p.attach_rot |= *rotation;
        }
        Module::ParticleRotation { rotation, scale, direct, spawn_only, emitter_time, module_time, .. } => {
            let time = if *emitter_time {
                t
            } else if *module_time {
                p.age
            } else {
                p.rel
            };
            let v = if *spawn_only { p.prot_sample } else { rotation.as_ref().map_or(Vec3::ZERO, |d| d.vec3(time, rng)) * scale.as_ref().map_or(Vec3::ONE, |d| d.vec3(time, rng)) };
            if *direct {
                p.prot_added = v;
            } else {
                p.prot_acc += v * dt;
                p.prot_added = p.prot_acc;
            }
        }
        Module::CurlNoise { intensity, position_scale, time_scale, axis_scale, particle_time, time_axis, no_offset } => {
            // Update 0x1442FC8C0 + 0x1442E1920 [C/I]: three samples of Perlin's improved noise
            // (0x1443B8F30, the reference permutation), each with the time added on its own axis,
            // turned by the node and added to the velocity of this tick.
            let ps = position_scale.float(t, rng);
            let time = time_scale.float(t, rng) * space.seconds;
            let s = axis_scale.as_ref().map_or(Vec3::ONE, |d| d.vec3(t, rng));
            let amount = intensity.float(if *particle_time { p.rel } else { t }, rng);
            let q = (p.loc + if *no_offset { Vec3::ZERO } else { space.noise_offset }) * ps;
            let n = match time_axis {
                0 => Vec3::new(perlin((q.x + time) * s.x, q.y * s.y, q.z * s.z), perlin(q.x * s.x, (q.y + time) * s.y, q.z * s.z), perlin(q.x * s.x, q.y * s.y, (q.z + time) * s.z)),
                // One axis: the three samples differ only by their time term, which is the same value.
                1 => Vec3::splat(perlin((q.x + time) * s.x, q.y * s.y, q.z * s.z)),
                2 => Vec3::splat(perlin(q.x * s.x, (q.y + time) * s.y, q.z * s.z)),
                _ => Vec3::splat(perlin(q.x * s.x, q.y * s.y, (q.z + time) * s.z)),
            };
            p.vel += space.rot * n * amount;
        }
        Module::Shape(shape) => {
            // `bAttachParticle` (Update 0x14437A760): the particle keeps its corner and fraction
            // as the ring's radius, offset and scale change.
            if let (ShapeKind::Ring { attach: true, .. }, true) = (&shape.kind, p.ring_vertex != u32::MAX) {
                if let Some(v) = ring_values(shape, t, rng) {
                    let direction = rotator(shape.direction.y, shape.direction.z, shape.direction.x);
                    let scale = if shape.location_scale { space.world_scale } else { Vec3::ONE };
                    let point = ring_point(&v, &direction, scale, p.ring_vertex, p.ring_fraction, p.ring_mix);
                    if p.is_local {
                        p.local = point;
                    } else {
                        p.loc = space.origin + space.rot * point;
                    }
                }
            }
        }
        Module::LocalParticle { .. } => {}
        // ---- (VFXR)
        Module::SphereForce { center, radius, radius_scale, internal, hit_size } => {
            let c = space.origin + space.rot * center.vec3(t, rng);
            let r = radius * radius_scale.float(t, rng) * space.world_scale.x.abs().max(1.0e-4);
            let d = p.loc - c;
            let dist = d.length();
            if dist > 1.0e-4 && r > 0.0 && ((*internal && dist > r) || (!*internal && dist < r)) {
                let n = d / dist;
                p.loc = c + n * r;
                let vn = p.vel.dot(n);
                // the velocity mirrored when it goes on through the surface
                if (*internal && vn > 0.0) || (!*internal && vn < 0.0) {
                    p.vel -= 2.0 * vn * n;
                    let bn = p.base_vel.dot(n);
                    p.base_vel -= 2.0 * bn * n;
                }
                if !p.hit_scaled {
                    p.hit_scaled = true;
                    p.size *= *hit_size;
                    p.base_size *= *hit_size;
                }
            }
        }
        Module::Attractor { position, range, strength, by_distance, world, affect_base, override_velocity, axes } => {
            let at = position.vec3(t, rng);
            let at = if *world { Vec3::from(space.to_world.inverse().transform_point3a(at.into())) } else { space.origin + space.rot * at };
            let dir = at - p.loc;
            let distance = dir.length();
            let range = range.float(t, rng);
            if distance <= range {
                let k = if *by_distance && range > 0.0 { strength.float((range - distance) / range, rng) } else { strength.float(p.rel, rng) };
                let mut delta = dir.normalize_or_zero() * k * dt;
                for a in 0..3 {
                    if (!axes[2 * a] && delta[a] > 0.0) || (!axes[2 * a + 1] && delta[a] < 0.0) {
                        delta[a] = 0.0;
                    }
                }
                if *override_velocity {
                    p.vel = delta;
                } else {
                    p.vel += delta;
                }
                if *affect_base {
                    p.base_vel += delta;
                }
            }
        }
        Module::AttractorGravity { position, radius, strength } => {
            let at = space.origin + space.rot * *position;
            let dir = at - p.loc;
            if dir.length_squared() <= radius * radius {
                let a = dir.normalize_or_zero() * strength.float(p.rel, rng) * dt;
                p.vel += a;
                p.base_vel += a;
            }
        }
        Module::Collision { damping, completion, .. } => {
            if let (Some(ground), false) = (space.ground, p.frozen) {
                let world = space.to_world.transform_point3(p.loc);
                let n = space.from_world(Vec3::Z).normalize_or(Vec3::Z);
                let h = world.z - ground;
                if h < 0.0 && p.vel.dot(n) < 0.0 && !(p.hits as f32 >= p.max_hits && *completion == 2) {
                    p.collided = true;
                    if p.hits as f32 >= p.max_hits {
                        match completion {
                            0 => p.dead = true,
                            1 => {
                                p.frozen = true;
                                p.vel = Vec3::ZERO;
                                p.base_vel = Vec3::ZERO;
                            }
                            _ => {}
                        }
                    } else {
                        p.hits += 1;
                        let k = damping.vec3(p.rel, rng);
                        let vn = p.vel.dot(n);
                        p.vel = (p.vel - 2.0 * vn * n) * k;
                        let bn = p.base_vel.dot(n);
                        p.base_vel = (p.base_vel - 2.0 * bn * n) * k;
                        p.loc -= n * h;
                    }
                }
            }
        }
        Module::CollisionGpu { resilience, resilience_scale, friction, radius_scale, radius_bias, response } => {
            if let Some(ground) = space.ground {
                let radius = p.size.x.abs() * 0.5 * radius_scale + radius_bias;
                let world = space.to_world.transform_point3(p.loc);
                let n = space.from_world(Vec3::Z).normalize_or(Vec3::Z);
                let h = world.z - (ground + radius);
                let vn = p.vel.dot(n);
                if h < 0.0 && vn < 0.0 {
                    p.collided = true;
                    p.loc -= n * h;
                    match response {
                        2 => p.dead = true,
                        1 => {
                            p.vel = Vec3::ZERO;
                            p.base_vel = Vec3::ZERO;
                        }
                        _ => {
                            let e = resilience.float(p.rel, rng) * resilience_scale.float(p.rel, rng);
                            let tangent = (p.vel - vn * n) * (1.0 - friction).clamp(0.0, 1.0);
                            p.vel = tangent - vn * e * n;
                            p.base_vel = p.vel;
                        }
                    }
                }
            }
        }
        Module::SizeBySpeed { speed_scale, max } => {
            let speed = p.vel.length();
            p.size.x *= (speed * speed_scale.x).clamp(1.0, max.x.max(1.0));
            p.size.y *= (speed * speed_scale.y).clamp(1.0, max.y.max(1.0));
        }
        Module::AccelOverLife { accel, world } => {
            let a = accel.vec3(p.rel, rng);
            let a = if *world { space.from_world(a) } else { space.vector(a) } * dt;
            p.vel += a;
            p.base_vel += a;
        }
        Module::ParticleLocation { location, start_scale, world, axes } => {
            let mut o = location.vec3(p.rel, rng) * start_scale.vec3(p.rel, rng);
            for a in 0..3 {
                if !axes[a] {
                    o[a] = 0.0;
                }
            }
            let o = if *world { space.from_world(o) } else { space.vector(o * space.world_scale) };
            if p.is_local {
                p.local += o - p.ploc_offset;
            } else {
                p.loc += o - p.ploc_offset;
            }
            p.ploc_offset = o;
        }
        Module::Lifetime(_) | Module::Size(_) | Module::Color { .. } | Module::Velocity { .. } | Module::Rotation(_) | Module::RotationRate(_) | Module::MeshRotation(_) | Module::MeshRotationRate(_) | Module::Location(_) | Module::Primitive(_) | Module::PivotOffset(_) | Module::Orbit { .. } | Module::DepthOffset(_) | Module::InitSizeScale | Module::MeshPivotOffset(_) => {}
    }
}

/// Ken Perlin's reference permutation: the table at 0x147763A50 of the executable is the same
/// 256 values, repeated.
const PERM: [u8; 256] = [
    151, 160, 137, 91, 90, 15, 131, 13, 201, 95, 96, 53, 194, 233, 7, 225, 140, 36, 103, 30, 69, 142, 8, 99, 37, 240, 21, 10, 23, 190, 6, 148, 247, 120, 234, 75, 0, 26, 197, 62, 94, 252, 219, 203, 117, 35, 11, 32, 57, 177, 33, 88, 237, 149, 56, 87,
    174, 20, 125, 136, 171, 168, 68, 175, 74, 165, 71, 134, 139, 48, 27, 166, 77, 146, 158, 231, 83, 111, 229, 122, 60, 211, 133, 230, 220, 105, 92, 41, 55, 46, 245, 40, 244, 102, 143, 54, 65, 25, 63, 161, 1, 216, 80, 73, 209, 76, 132, 187, 208, 89, 18,
    169, 200, 196, 135, 130, 116, 188, 159, 86, 164, 100, 109, 198, 173, 186, 3, 64, 52, 217, 226, 250, 124, 123, 5, 202, 38, 147, 118, 126, 255, 82, 85, 212, 207, 206, 59, 227, 47, 16, 58, 17, 182, 189, 28, 42, 223, 183, 170, 213, 119, 248, 152, 2, 44,
    154, 163, 70, 221, 153, 101, 155, 167, 43, 172, 9, 129, 22, 39, 253, 19, 98, 108, 110, 79, 113, 224, 232, 178, 185, 112, 104, 218, 246, 97, 228, 251, 34, 242, 193, 238, 210, 144, 12, 191, 179, 162, 241, 81, 51, 145, 235, 249, 14, 239, 107, 49, 192,
    214, 31, 181, 199, 106, 157, 184, 84, 204, 176, 115, 121, 50, 45, 127, 4, 150, 254, 138, 236, 205, 93, 222, 114, 67, 29, 24, 72, 243, 141, 128, 195, 78, 66, 215, 61, 156, 180,
];

/// Perlin's improved noise (0x1443B8F30 [C]: quintic fade, the 16-case gradient, range about
/// -1..1).
pub fn perlin(x: f32, y: f32, z: f32) -> f32 {
    let p = |i: usize| PERM[i & 255] as usize;
    let (fx, fy, fz) = (x.floor(), y.floor(), z.floor());
    let (xi, yi, zi) = ((fx as i32 & 255) as usize, (fy as i32 & 255) as usize, (fz as i32 & 255) as usize);
    let (x, y, z) = (x - fx, y - fy, z - fz);
    let fade = |t: f32| t * t * t * (t * (t * 6.0 - 15.0) + 10.0);
    let (u, v, w) = (fade(x), fade(y), fade(z));
    let grad = |hash: usize, x: f32, y: f32, z: f32| {
        let h = hash & 15;
        let u = if h < 8 { x } else { y };
        let v = if h < 4 {
            y
        } else if h == 12 || h == 14 {
            x
        } else {
            z
        };
        (if h & 1 == 0 { u } else { -u }) + (if h & 2 == 0 { v } else { -v })
    };
    let lerp = |t: f32, a: f32, b: f32| a + t * (b - a);
    let a = p(xi) + yi;
    let (aa, ab) = (p(a) + zi, p(a + 1) + zi);
    let b = p(xi + 1) + yi;
    let (ba, bb) = (p(b) + zi, p(b + 1) + zi);
    lerp(
        w,
        lerp(v, lerp(u, grad(p(aa), x, y, z), grad(p(ba), x - 1.0, y, z)), lerp(u, grad(p(ab), x, y - 1.0, z), grad(p(bb), x - 1.0, y - 1.0, z))),
        lerp(v, lerp(u, grad(p(aa + 1), x, y, z - 1.0), grad(p(ba + 1), x - 1.0, y, z - 1.0)), lerp(u, grad(p(ab + 1), x, y - 1.0, z - 1.0), grad(p(bb + 1), x - 1.0, y - 1.0, z - 1.0))),
    )
}

// -------------------------------------------------------------------------------------- drawing

fn mul_color(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    [a[0] * b[0], a[1] * b[1], a[2] * b[2], a[3] * b[3]]
}

fn sub_rect(index: u32, h: u32, v: u32) -> [f32; 4] {
    let (col, row) = (index % h, (index / h).min(v - 1));
    [col as f32 / h as f32, row as f32 / v as f32, (col + 1) as f32 / h as f32, (row + 1) as f32 / v as f32]
}

/// The rotation that turns a mesh (or node) toward the camera: mesh +X points at the viewer,
/// +Y along the camera's right, +Z along its up.
fn facing(billboard: Billboard, base: &Mat3, position: Vec3, camera: &Camera) -> Mat3 {
    match billboard {
        Billboard::None => *base,
        Billboard::Screen => Mat3::from_cols(-camera.forward, camera.right, camera.up),
        Billboard::Camera => {
            let x = (camera.position - position).normalize_or_zero();
            if x == Vec3::ZERO {
                return Mat3::from_cols(-camera.forward, camera.right, camera.up);
            }
            let y = camera.up.cross(x).normalize_or_zero();
            let y = if y == Vec3::ZERO { camera.right } else { -y };
            let z = y.cross(x);
            Mat3::from_cols(x, y, if z.dot(camera.up) < 0.0 { -z } else { z })
        }
        Billboard::AxisX | Billboard::AxisY | Billboard::AxisZ => {
            // Turns about its own axis so that another of its axes looks at the camera.
            let (axis, k) = match billboard {
                Billboard::AxisX => (base.x_axis, 0),
                Billboard::AxisY => (base.y_axis, 1),
                _ => (base.z_axis, 2),
            };
            let to = camera.position - position;
            let front = (to - axis * to.dot(axis)).normalize_or_zero();
            if front == Vec3::ZERO {
                return *base;
            }
            let side = axis.cross(front);
            match k {
                0 => Mat3::from_cols(axis, side, front),
                1 => Mat3::from_cols(front, axis, side),
                _ => Mat3::from_cols(front, side, axis),
            }
        }
    }
}

// (IMPL_ER20) Two rules of the mesh / sprite placement that hosts may switch back (diagnosis):
// bit 0 = the depth offset as before this pass (negative = nearer), bit 1 = no lens rule.
static PLACEMENT_SWITCHES: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

/// `SQEX_DepthOffset`: POSITIVE = toward the camera (see `def::Module::DepthOffset`). `false`
/// restores the reading before IMPL_ER20 (negative = nearer).
pub fn set_depth_offset_toward_camera(on: bool) {
    if on {
        PLACEMENT_SWITCHES.fetch_and(!1, std::sync::atomic::Ordering::Relaxed);
    } else {
        PLACEMENT_SWITCHES.fetch_or(1, std::sync::atomic::Ordering::Relaxed);
    }
}

/// The lens rule of `def::EmitterDef::lens_rule` on / off.
pub fn set_lens_rule(on: bool) {
    if on {
        PLACEMENT_SWITCHES.fetch_and(!2, std::sync::atomic::Ordering::Relaxed);
    } else {
        PLACEMENT_SWITCHES.fetch_or(2, std::sync::atomic::Ordering::Relaxed);
    }
}

fn depth_sign() -> f32 {
    if PLACEMENT_SWITCHES.load(std::sync::atomic::Ordering::Relaxed) & 1 == 0 { 1.0 } else { -1.0 }
}

/// (IMPL_ER20) Whether the lens rule turns this particle: see `def::EmitterDef::lens_rule`.
pub fn lens_turns(rule: bool, size: Vec3) -> bool {
    rule && PLACEMENT_SWITCHES.load(std::sync::atomic::Ordering::Relaxed) & 2 == 0 && size.x > 0.0 && (size.x - size.z).abs() <= 1.0e-3 * size.x && size.y.abs() < 0.35 * size.x
}

fn only_emitters() -> Option<&'static Vec<String>> {
    static ONLY: std::sync::OnceLock<Option<Vec<String>>> = std::sync::OnceLock::new();
    ONLY.get_or_init(|| std::env::var("KH_VFX_EMITTER").ok().map(|v| v.split(',').map(str::to_string).collect())).as_ref()
}

/// (INT) An AnimTrail ribbon's drawn head (both sockets) `alpha` of the way from the sample of the step before to the
/// newest one, when the newest was taken at the latest step (its age is 0 until the next step); else the newest sample.
fn trail_head_at(particles: &[Particle], alpha: f32) -> Option<(Vec3, Vec3)> {
    let n = particles.len();
    let newest = particles.get(n.checked_sub(1)?)?;
    match n.checked_sub(2).and_then(|k| particles.get(k)) {
        Some(before) if alpha < 1.0 && newest.age == 0.0 => {
            // the second socket (the blade's base, by the hand) between the two samples; the first (the tip) turned about
            // it as the blade turns, as the drawn hand's rotation is blended (a chord would cut inside a fast swing's arc)
            let edge = before.edge.lerp(newest.edge, alpha);
            let (d0, d1) = (before.loc - before.edge, newest.loc - newest.edge);
            let (l0, l1) = (d0.length(), d1.length());
            let loc = if l0 > 1.0e-3 && l1 > 1.0e-3 {
                let turn = glam::Quat::from_rotation_arc(d0 / l0, d1 / l1);
                edge + glam::Quat::IDENTITY.slerp(turn, alpha) * (d0 / l0) * (l0 + (l1 - l0) * alpha)
            } else {
                before.loc.lerp(newest.loc, alpha)
            };
            Some((loc, edge))
        }
        _ => Some((newest.loc, newest.edge)),
    }
}

/// (INT) A component `alpha` of the way between two steps' as a rigid transform: the place and the scale lerped, the
/// rotation slerped (a component's rotation is a rotation: its axes may carry the scale, taken out first).
pub fn rigid_blend(a: &Xform, b: &Xform, alpha: f32) -> Affine3A {
    let q = |m: &Mat3| glam::Quat::from_mat3(&Mat3::from_cols(m.x_axis.normalize_or(Vec3::X), m.y_axis.normalize_or(Vec3::Y), m.z_axis.normalize_or(Vec3::Z))).normalize();
    let rotation = Mat3::from_quat(q(&a.rotation).slerp(q(&b.rotation), alpha));
    Xform { position: a.position.lerp(b.position, alpha), rotation, scale: a.scale.lerp(b.scale, alpha) }.affine()
}

/// (INT) The component between the steps and the two steps' inverses: the host's drawn component when it gave one, else
/// the rigid blend (None: it stood still, or the latest step is drawn).
fn comp_blend(s: &System, alpha: f32) -> Option<(Affine3A, Affine3A, Affine3A)> {
    if alpha >= 1.0 {
        return None;
    }
    let c = match (s.drawn_component, s.draw_from) {
        (Some(d), _) => d.affine(),
        (None, Some(from)) => rigid_blend(&from, &s.xform, alpha),
        (None, None) => return None,
    };
    let (x0, x1) = (s.draw_from.unwrap_or(s.xform).affine().inverse(), s.xform.affine().inverse());
    (c.is_finite() && x0.is_finite() && x1.is_finite()).then_some((c, x0, x1))
}

/// (INT) An AnimTrail's drawn head: the host's drawn sockets while the newest sample is the latest step's, else the blend.
fn trail_head(s: &System, particles: &[Particle], alpha: f32) -> Option<(Vec3, Vec3)> {
    match (s.drawn_trail, particles.last()) {
        (Some(d), Some(newest)) if alpha < 1.0 && newest.age == 0.0 && particles.len() >= 2 => Some(d),
        _ => trail_head_at(particles, alpha),
    }
}

fn draw_system(s: &System, ids: &[(u32, u32, u32)], sub_ids: &[Vec<u32>], alpha: f32, camera: &Camera, out: &mut DrawList) {
    let fade = s.fade.clamp(0.0, 1.0);
    // (INT) the component between the steps and the two steps' inverses (None: it stood still, or the latest step is drawn)
    let comp_blend = comp_blend(s, alpha);
    let system_color = mul_color(s.color, s.def.base_color);
    // (VFXR sort) each emitter's draws become one group per kind (`DrawGroup`), its particles ordered by its SortMode;
    // a group is closed when the next emitter starts and after the last
    let mut pending: Option<GroupOpen> = None;
    for e in &s.emitters {
        if let Some(p) = pending.take() {
            close_group(out, s, camera, p);
        }
        if e.done || !e.started {
            continue;
        }
        let d = &s.def.emitters[e.def];
        pending = Some(GroupOpen { marks: [out.quads.len(), out.ribbons.len(), out.meshes.len(), out.decals.len()], emitter: e.def as u32, sort: d.sort_mode });
        // Diagnosis: KH_VFX_EMITTER=a,b draws only the emitters whose name contains one of the words.
        if let Some(only) = only_emitters() {
            if !only.iter().any(|w| d.name.contains(w.as_str())) {
                continue;
            }
        }
        let (material, mesh, decal_material) = ids.get(e.def).copied().unwrap_or((NO_ID, NO_ID, NO_ID));
        // (INT) on a moving component: the component blended as a rigid transform x the node's place in it blended
        let blend = |prev: &Affine3A, now: &Affine3A| match comp_blend {
            Some((c, x0, x1)) => c * lerp_affine(&(x0 * *prev), &(x1 * *now), alpha),
            None => lerp_affine(prev, now, alpha),
        };
        let node = blend(&e.node_prev, &e.node);
        // Stock [U]: the renderer multiplies a particle's size by the component's scale (the
        // game's own node scale is already in the size, through the emitter's size multiplier).
        // With `bUseEmitterLocalSpace` a mesh is drawn through the node's full matrix (0x144284A80 [C]).
        let node_scale = e.comp_scale;
        let mesh_scale = if d.emitter_local { scale_of(&node) } else { e.comp_scale };
        let sim = if d.local_space { blend(&e.sim_prev, &e.sim) } else { Affine3A::IDENTITY };
        let node_rot = Mat3::from_cols(Vec3::from(node.matrix3.x_axis).normalize_or_zero(), Vec3::from(node.matrix3.y_axis).normalize_or_zero(), Vec3::from(node.matrix3.z_axis).normalize_or_zero());
        let tint = if d.inherit.fade { [system_color[0], system_color[1], system_color[2], system_color[3] * fade] } else { system_color };
        let world_of = |p: &Particle| {
            let l = p.prev_loc.lerp(p.loc, alpha);
            let l = if d.local_space { sim.transform_point3(l) } else { l };
            if p.orbit != Vec3::ZERO {
                l + node_rot * p.orbit
            } else {
                l
            }
        };
        // ParticleModuleLight (stock [U]): colour = particle colour x alpha x ColorScaleOverLife x
        // BrightnessOverLife, radius = RadiusScale x the particle's size.
        if let Some(light) = &d.light {
            let mut rng = Rng::new(1);
            for p in e.particles.iter().take(4) {
                let c = light.color.vec3(p.rel, &mut rng) * light.brightness.float(p.rel, &mut rng) * p.color[3].max(0.0) * fade;
                let color = Vec3::new(p.color[0], p.color[1], p.color[2]) * c * Vec3::new(system_color[0], system_color[1], system_color[2]);
                let radius = light.radius.float(p.rel, &mut rng) * 0.5 * (p.size.x + if p.size.y != 0.0 { p.size.y } else { p.size.x }) * node_scale.x;
                if radius > 0.0 && color.max_element() > 0.0 {
                    out.lights.push(Light { position: world_of(p), color, radius, exponent: light.exponent.float(p.rel, &mut rng).max(0.0), characters: s.char_lights });
                }
            }
        }
        // EmitterRenderMode ERM_LightsOnly / ERM_None: nothing of the emitter itself is drawn.
        if !d.render {
            continue;
        }
        match d.kind {
            Kind::Control | Kind::Unsupported => {}
            Kind::Sprite => {
                if material == NO_ID {
                    continue;
                }
                let frames = d.sub_h * d.sub_v;
                for p in e.particles.iter().take(d.max_draw) {
                    let color = mul_color(p.color, tint);
                    if color[3] <= 0.0 || p.size.x == 0.0 {
                        continue;
                    }
                    let mut c = world_of(p);
                    let to_camera = (camera.position - c).normalize_or_zero();
                    c += to_camera * (p.cam_offset + depth_sign() * p.depth_offset);
                    let hx = p.size.x * 0.5 * node_scale.x;
                    // Stock sprite size [U]: only PSA_Square and PSA_FacingCameraPosition copy X into Y; every
                    // other alignment (also PSA_TypeSpecific: the fire ball's flare is 8000 x 60) is X by Y.
                    let hy = if !matches!(d.alignment, Alignment::Square | Alignment::FacingCameraPosition) && p.size.y != 0.0 { p.size.y * 0.5 * node_scale.x } else { hx };
                    let (mut right, mut up) = (camera.right, camera.up);
                    match d.alignment {
                        Alignment::Velocity | Alignment::AwayFromCenter => {
                            let along = if d.alignment == Alignment::Velocity {
                                if d.local_space {
                                    sim.transform_vector3(p.vel)
                                } else {
                                    p.vel
                                }
                            } else {
                                c - Vec3::from(node.translation)
                            };
                            let along = along.normalize_or_zero();
                            let side = along.cross(to_camera).normalize_or_zero();
                            if along != Vec3::ZERO && side != Vec3::ZERO {
                                up = along;
                                right = side;
                            }
                        }
                        _ => {}
                    }
                    let rot = if p.has_prot { (p.prot_start.x + p.prot_added.x).to_radians() } else { p.rot };
                    if rot != 0.0 {
                        let (sin, cos) = rot.sin_cos();
                        let (r, u) = (right * cos + up * sin, up * cos - right * sin);
                        right = r;
                        up = u;
                    }
                    // Pivot offset: fractions of the sprite's size.
                    c += right * (e.pivot.x * 2.0 * hx) - up * (e.pivot.y * 2.0 * hy);
                    let (r, u) = (right * hx, up * hy);
                    let f = p.sub.clamp(0.0, (frames - 1) as f32);
                    let i0 = f.floor() as u32;
                    let i1 = (i0 + 1).min(frames - 1);
                    out.quads.push(Quad { corners: [c - r + u, c + r + u, c + r - u, c - r - u], uv: sub_rect(i0, d.sub_h, d.sub_v), uv2: sub_rect(i1, d.sub_h, d.sub_v), blend: if d.sub_blend { f - i0 as f32 } else { 0.0 }, color, material, dynamic: p.dynamic, time: e.time, rel: p.rel, center: c, size: 2.0 * hx, mode: d.blend });
                }
            }
            Kind::Mesh | Kind::Fragment => {
                // (XW-d) a fragment mesh draws its pieces, never its placeholder `Mesh` (the axis gizmo tmd_xyz_00)
                if mesh == NO_ID && d.kind == Kind::Mesh {
                    continue;
                }
                for p in e.particles.iter().take(d.max_draw) {
                    let color = mul_color(p.color, tint);
                    if color[3] <= 0.0 {
                        continue;
                    }
                    let position = world_of(p);
                    let base = if p.shape_rot && !p.attach_rot {
                        if d.local_space {
                            unit_axes(&sim) * p.spawn_rot
                        } else {
                            p.spawn_rot
                        }
                    } else if d.local_space || p.attach_rot {
                        node_rot
                    } else {
                        p.spawn_rot
                    };
                    // (fourth pass) An axis billboard turns about the component's axis unless the
                    // type data asks for the node's rotated one; the offset turns the mesh inside
                    // the billboard frame (Thunder's ground spikes: a flat grid pitched upright).
                    let sqex_billboard = matches!(d.mesh_billboard, Billboard::AxisX | Billboard::AxisY | Billboard::AxisZ | Billboard::Screen) || (d.mesh_billboard == Billboard::Camera && d.billboard_offset != Vec3::ZERO);
                    let axis_base = if matches!(d.mesh_billboard, Billboard::AxisX | Billboard::AxisY | Billboard::AxisZ) && !d.billboard_node_axis { s.xform.rotation } else { base };
                    let mut base = facing(d.mesh_billboard, &axis_base, position, camera);
                    if sqex_billboard && d.billboard_offset != Vec3::ZERO {
                        base = base * crate::math::rotator(d.billboard_offset.x, d.billboard_offset.y, d.billboard_offset.z);
                    }
                    // (IMPL_ER20) The lens rule: the flat axis of a camera-facing ball looks at the viewer.
                    if lens_turns(d.lens_rule, p.size) {
                        base = base * crate::math::rotator(0.0, 90.0, 0.0);
                    }
                    let mut rotation = base * euler(p.mesh_rot);
                    if p.has_prot {
                        // R(start) x R(added) (0x1442EF550 [C]); UE row order: start first.
                        rotation = rotation * euler(p.prot_added) * euler(p.prot_start);
                    }
                    let scale = p.size * mesh_scale;
                    let m = rotation * Mat3::from_diagonal(scale);
                    let to_camera = (camera.position - position).normalize_or_zero();
                    let position = position + to_camera * (p.cam_offset + depth_sign() * p.depth_offset) + m * e.mesh_pivot;
                    // (XW-d) TypeDataSQEX_FragmentMesh [?]: every piece at its offset, flying apart from ImpactTime on (def::FragmentDef)
                    if let (Kind::Fragment, Some(f)) = (d.kind, d.fragments.as_ref()) {
                        let list = sub_ids.get(e.def).map(Vec::as_slice).unwrap_or(&[]);
                        let age = p.prev_age + (p.age - p.prev_age) * alpha;
                        let flight = (age - f.impact_time).max(0.0);
                        for (k, piece) in f.pieces.iter().enumerate() {
                            let Some(&piece_mesh) = piece.mesh.and_then(|i| list.get(i)) else { continue };
                            if piece_mesh == NO_ID {
                                continue;
                            }
                            let mut r = Rng::new(p.seed ^ (k as u32 + 1).wrapping_mul(0x9E37_79B9));
                            let place = piece.translation * (f.location_scale.float(p.rel, &mut r) + f.random_location_scale.float(p.rel, &mut r));
                            let mut offset = m * place;
                            let mut turn = Mat3::IDENTITY;
                            if flight > 0.0 {
                                let away = (place - f.impact_location).normalize_or_zero();
                                let speed = f.impact_power + f.random_impact_power * r.f32();
                                let push = rotation * (away * speed) + if f.impact_vector_world { f.impact_vector } else { rotation * f.impact_vector };
                                let accel = Vec3::new(0.0, 0.0, f.gravity) + rotation * f.force;
                                offset += push * flight + accel * (0.5 * flight * flight);
                                let spin = f.impact_rotation.as_ref().map_or(Vec3::ZERO, |s| s.vec3(p.rel, &mut r));
                                turn = euler(spin * flight);
                            }
                            let pm = m * Mat3::from_quat(piece.rotation) * turn * Mat3::from_diagonal(piece.scale);
                            out.meshes.push(MeshInstance { mesh: piece_mesh, material, transform: Affine3A::from_mat3_translation(pm, position + offset), color, dynamic: p.dynamic, time: e.time, rel: p.rel, blend: d.blend, uv_mode: d.mesh_uv_mode, sub: crate::draw::sub_image_code(p.sub, d.sub_h, d.sub_v, d.sub_blend) /* (ZA) */ });
                        }
                        continue;
                    }
                    // Sub meshes [?]: see def::SubMeshes.
                    let mesh = match (&d.sub_meshes, sub_ids.get(e.def)) {
                        (Some(sm), Some(list)) if !list.is_empty() => {
                            let n = list.len() as f32;
                            let fraction = if sm.interval > 0.0 {
                                let x = if sm.emitter_time { e.rel } else { p.age } / sm.interval;
                                let x = if sm.no_loop { x.min(1.0) } else { x.fract() };
                                sm.index.float(x, &mut Rng::new(1)) + p.mesh_phase
                            } else {
                                p.mesh_phase
                            };
                            let k = (fraction * n).floor();
                            list[(if sm.no_loop && sm.interval > 0.0 { k.min(n - 1.0) } else { k.rem_euclid(n) }) as usize % list.len()]
                        }
                        _ => mesh,
                    };
                    out.meshes.push(MeshInstance { mesh, material, transform: Affine3A::from_mat3_translation(m, position), color, dynamic: p.dynamic, time: e.time, rel: p.rel, blend: d.blend, uv_mode: d.mesh_uv_mode, sub: crate::draw::sub_image_code(p.sub, d.sub_h, d.sub_v, d.sub_blend) /* (ZA) */ });
                }
            }
            Kind::AnimTrail => {
                if material == NO_ID || e.particles.len() < 2 {
                    continue;
                }
                // Newest last in the particle list: the head is the last sample.
                let n = e.particles.len();
                let mut points = Vec::with_capacity(n);
                // (INT) the head between the two latest samples by `alpha`, where the drawn blade is (before: the latest
                // sample, up to a step ahead of the blade drawn between the steps)
                let head = trail_head(s, &e.particles, alpha);
                for (k, p) in e.particles.iter().enumerate().rev() {
                    let (loc, edge) = match head {
                        Some(h) if k == n - 1 => h,
                        _ => (p.loc, p.edge),
                    };
                    let offset = (camera.position - loc).normalize_or_zero() * (p.cam_offset + depth_sign() * p.depth_offset);
                    points.push(RibbonPoint { a: loc + offset, b: edge + offset, u: if k == n - 1 { 0.0 } else { p.rel.clamp(0.0, 1.0) }, color: mul_color(p.color, tint) });
                }
                out.ribbons.push(Ribbon { points, material, dynamic: e.particles[0].dynamic, time: e.time, blend: d.blend, tiles: 1.0, v_scale: 1.0, uv_turned: false });
            }
            Kind::Tail | Kind::TailSprites => {
                if material == NO_ID {
                    continue;
                }
                let Some(tail) = &d.tail else { continue };
                let Some(p) = e.particles.first() else { continue };
                let mut rng = Rng::new(1);
                let n = tail.number.float(e.rel, &mut rng).round().clamp(2.0, 400.0) as usize;
                // 0x144287CC0 [C]: TailInterval x |component scale|.
                let interval = (tail.interval.float(e.rel, &mut rng) * s.xform.scale.x.abs()).max(0.01);
                // The path, newest first, with the head between the two latest steps.
                let Some((&newest, older)) = e.path.split_last() else { continue };
                let head = e.path_prev_head.lerp(newest, alpha);
                let mut spots: Vec<Vec3> = Vec::with_capacity(n);
                spots.push(head);
                let mut from = head;
                let mut travelled = 0.0f32;
                'walk: for &to in older.iter().rev() {
                    let seg = from.distance(to);
                    if seg > 1.0e-6 {
                        // Points that fall on this stretch of the path.
                        while (spots.len() as f32) * interval <= travelled + seg {
                            let t = ((spots.len() as f32) * interval - travelled) / seg;
                            spots.push(from.lerp(to, t));
                            if spots.len() >= n {
                                break 'walk;
                            }
                        }
                        travelled += seg;
                    }
                    from = to;
                }
                let base = mul_color(p.color, tint);
                let scroll_at = tail.scroll.as_ref().map(|sc| e.time * sc.time_scale);
                let along = |k: usize, rng: &mut Rng| -> (f32, f32, [f32; 4]) {
                    let f = k as f32 / (n - 1) as f32;
                    let mut scale = tail.scale.float(f, rng);
                    let mut c = tail.color.as_ref().map_or(Vec3::ONE, |x| x.vec3(f, rng));
                    let mut a = tail.alpha.as_ref().map_or(1.0, |x| x.float(f, rng));
                    if let (Some(sc), Some(at)) = (&tail.scroll, scroll_at) {
                        let g = (f + at).rem_euclid(1.0);
                        scale *= sc.scale.as_ref().map_or(1.0, |x| x.vec3(g, rng).x);
                        c *= sc.color.as_ref().map_or(Vec3::ONE, |x| x.vec3(g, rng));
                        a *= sc.alpha.as_ref().map_or(1.0, |x| x.float(g, rng));
                    }
                    if d.kind == Kind::Tail {
                        c *= tail.polygon_color.as_ref().map_or(Vec3::ONE, |x| x.vec3(f, rng));
                        a *= tail.polygon_alpha.as_ref().map_or(1.0, |x| x.float(f, rng));
                    }
                    (f, scale, mul_color(base, [c.x, c.y, c.z, a]))
                };
                if d.kind == Kind::TailSprites {
                    // One sprite on every point (the tail's particles are ordinary sprites).
                    for (k, &spot) in spots.iter().enumerate() {
                        let (f, scale, color) = along(k, &mut rng);
                        let hx = 0.5 * p.size.x * node_scale.x * scale;
                        let hy = if !matches!(d.alignment, Alignment::Square | Alignment::FacingCameraPosition) && p.size.y != 0.0 { 0.5 * p.size.y * node_scale.x * scale } else { hx };
                        if hx <= 0.0 || color[3] <= 0.0 {
                            continue;
                        }
                        let to_camera = (camera.position - spot).normalize_or_zero();
                        let c = spot + to_camera * (p.cam_offset + depth_sign() * p.depth_offset);
                        let (r, u) = (camera.right * hx, camera.up * hy);
                        out.quads.push(Quad { corners: [c - r + u, c + r + u, c + r - u, c - r - u], uv: [0.0, 0.0, 1.0, 1.0], uv2: [0.0, 0.0, 1.0, 1.0], blend: 0.0, color, material, dynamic: p.dynamic, time: e.time, rel: f, center: c, size: 2.0 * hx, mode: d.blend });
                    }
                    continue;
                }
                if spots.len() < 2 {
                    continue;
                }
                let mut points = Vec::with_capacity(spots.len());
                for (k, &here) in spots.iter().enumerate() {
                    let (f, scale, color) = along(k, &mut rng);
                    let half = 0.5 * p.size.x.max(0.0) * node_scale.x * scale;
                    let dir = if k + 1 < spots.len() { here - spots[k + 1] } else { spots[k - 1] - here };
                    // RenderAxis: the sheet faces the camera (Trails_CameraUp) or stands on world up.
                    let side = if tail.world_up { dir.cross(Vec3::Z).cross(dir).normalize_or(Vec3::Z) } else { dir.cross(camera.position - here).normalize_or_zero() } * half;
                    let offset = (camera.position - here).normalize_or_zero() * (p.cam_offset + depth_sign() * p.depth_offset);
                    let u = if tail.u_reverse { 1.0 - f } else { f } * tail.u_scale;
                    let (a, b) = if tail.v_reverse { (here - side, here + side) } else { (here + side, here - side) };
                    points.push(RibbonPoint { a: a + offset, b: b + offset, u, color });
                }
                out.ribbons.push(Ribbon { points, material, dynamic: p.dynamic, time: e.time, blend: d.blend, tiles: 1.0, v_scale: tail.v_scale, uv_turned: tail.uv_turned });
            }
            Kind::Decal => {
                let Some(decal) = &d.decal else { continue };
                if decal_material == NO_ID || e.ended {
                    continue;
                }
                let mut rng = Rng::new(1);
                // 0x144281110 [C]: every curve of the type data is sampled at the emitter time; the
                // size is DecalSize x the node's world scale, the half extents of a stock decal box.
                let u = e.rel;
                let mut size = decal.size.vec3(u, &mut rng) * scale_of(&node);
                let c = decal.color.as_ref().map_or(Vec3::ONE, |x| x.vec3(u, &mut rng));
                let a = decal.opacity.as_ref().map_or(1.0, |x| x.float(u, &mut rng));
                let color = mul_color([c.x, c.y, c.z, a], tint);
                if color[3] <= 0.0 || size.min_element() <= 0.0 {
                    continue;
                }
                let turn = decal.rotation.as_ref().map_or(Mat3::IDENTITY, |x| euler(x.vec3(u, &mut rng)));
                if decal.scale_rot {
                    // bItUseScaleRotMatrixTransfrom: |size x R|, the size is given in the node's axes.
                    size = (turn * size).abs();
                }
                let m = node_rot * turn * Mat3::from_diagonal(size);
                let custom = [0, 1, 2].map(|k| decal.custom[k].as_ref().map_or(0.0, |x| x.float(u, &mut rng)));
                out.decals.push(Decal { material: decal_material, transform: Affine3A::from_mat3_translation(m, Vec3::from(node.translation)), color, custom, z_fade: decal.z_fade, time: e.time });
            }
        }
    }
    if let Some(p) = pending.take() {
        close_group(out, s, camera, p); // (VFXR sort)
    }
}

/// Diagnosis: one line per live emitter instance of an effect.
#[derive(Clone, Debug)]
pub struct EmitterProbe {
    pub name: String,
    pub parent: Option<usize>,
    pub rel: f32,
    pub position: Vec3,
    pub scale: Vec3,
    pub particles: usize,
    /// Largest particle size (x of size times the node scale that is applied when drawing).
    pub max_size: f32,
    /// Farthest particle from the node, cm.
    pub max_distance: f32,
    pub done: bool,
}

impl World {
    pub fn probe(&self, id: EffectId) -> Vec<EmitterProbe> {
        let Some(s) = self.systems.iter().find(|s| s.id == id) else { return Vec::new() };
        s.emitters
            .iter()
            .map(|e| {
                let d = &s.def.emitters[e.def];
                let scale = scale_of(&e.node);
                let draw_scale = if d.inherit.particle_size { scale } else { Vec3::ONE };
                let origin = Vec3::from(e.node.translation);
                let world = |p: &Particle| if d.local_space { e.node.transform_point3(p.loc) } else { p.loc };
                EmitterProbe {
                    name: d.name.clone(),
                    parent: e.parent,
                    rel: e.rel,
                    position: origin,
                    scale,
                    particles: e.particles.len(),
                    max_size: e.particles.iter().map(|p| (p.size * draw_scale).abs().max_element()).fold(0.0, f32::max),
                    max_distance: e.particles.iter().map(|p| world(p).distance(origin)).fold(0.0, f32::max),
                    done: e.done,
                }
            })
            .collect()
    }
}

/// Diagnosis (fourth pass): what is still alive, one entry per system.
#[derive(Clone, Debug)]
pub struct LiveSystem {
    pub id: EffectId,
    pub name: String,
    /// Loop end was requested (`stop(id, false)`), or a fade-out is running.
    pub ending: bool,
    pub seconds: f32,
    /// (emitter, emitter time, timeline ended, particles) of the instances that are not done.
    pub emitters: Vec<(String, f32, bool, usize)>,
}

impl World {
    /// (fourth pass) The scene's light scale the live effects ask for (`SQEX_ChangeSceneColor`
    /// LightIntensity at the emitter's time; the darkest wins): 1 = unchanged. The host scales its
    /// scene lighting by it (Thunder darkens the scene to 0.3 while it strikes).
    pub fn scene_light(&self) -> f32 {
        let mut value = 1.0f32;
        let mut rng = Rng::new(1);
        for s in self.systems.iter().filter(|s| !s.dead && s.delay <= 0.0) {
            for e in s.emitters.iter().filter(|e| e.started && !e.done) {
                if let Some((dist, _)) = &s.def.emitters[e.def].scene_light {
                    value = value.min(dist.float(e.rel, &mut rng).clamp(0.0, 1.0));
                }
            }
        }
        value
    }

    pub fn live(&self) -> Vec<LiveSystem> {
        self.systems
            .iter()
            .filter(|s| !s.dead)
            .map(|s| LiveSystem {
                id: s.id,
                name: s.def.name.clone(),
                ending: s.loop_end || s.fade_rate < 0.0,
                seconds: s.seconds,
                emitters: s.emitters.iter().filter(|e| !e.done).map(|e| (s.def.emitters[e.def].name.clone(), e.rel, e.ended, e.particles.len())).collect(),
            })
            .collect()
    }
}

#[cfg(test)]
mod zetta_primitive_tests {
    //! (ZETTA) the stock location primitives (Zettaflare's circle: sparks on a 280 cm ring at his feet, dust thrown outward)
    use super::*;

    fn cyl(radius: f32, height: f32, surface: bool, velocity: bool) -> Primitive {
        Primitive { cylinder: true, radius: crate::dist::Dist::constant(radius), height: crate::dist::Dist::constant(height), height_axis: 2, radial_velocity: true, axes: [true, true, true, true, false, false], surface_only: surface, velocity, velocity_scale: crate::dist::Dist::constant(1.0), start_location: crate::dist::Dist::constant_vec(Vec3::ZERO) }
    }

    #[test]
    fn a_surface_cylinder_puts_every_particle_on_its_ring() {
        let mut rng = Rng::new(3);
        let pr = cyl(280.0, 50.0, true, false);
        for _ in 0..500 {
            let (o, v) = primitive(&pr, 0.0, &mut rng);
            assert!((Vec2::new(o.x, o.y).length() - 280.0).abs() < 0.5, "{o}");
            assert_eq!(o.z, 0.0, "Z halves off: on the ground ring");
            assert!(v.is_none());
        }
    }

    #[test]
    fn a_filled_cylinder_stays_inside_and_pushes_radially() {
        let mut rng = Rng::new(5);
        let mut pr = cyl(400.0, 100.0, false, true);
        pr.axes = [true; 6];
        for _ in 0..500 {
            let (o, v) = primitive(&pr, 0.0, &mut rng);
            assert!(Vec2::new(o.x, o.y).length() <= 400.0 + 1e-3 && o.z.abs() <= 50.0 + 1e-3, "{o}");
            let v = v.expect("velocity");
            assert!(v.z.abs() < 1e-5, "radial velocity has no height part");
            assert!(v.length() <= 1.0 + 1e-4);
        }
    }

    #[test]
    fn a_sphere_on_its_surface_with_positive_z_only() {
        let mut rng = Rng::new(9);
        let pr = Primitive { cylinder: false, axes: [true, true, true, true, true, false], ..cyl(25.0, 0.0, true, true) };
        for _ in 0..500 {
            let (o, _) = primitive(&pr, 0.0, &mut rng);
            assert!((o.length() - 25.0).abs() < 0.01 && o.z >= 0.0, "{o}");
        }
    }
}

// ---------------------------------------------------------------------------------------------- (VFXR sort) draw groups
/// One emitter's draws begun: where each kind's list stood, the emitter and its SortMode.
struct GroupOpen {
    marks: [usize; 4],
    emitter: u32,
    sort: crate::def::SortMode,
}

/// The emitter's draws since `p.marks` become its groups (one per kind with draws), its quads and meshes ordered by its
/// SortMode first (`FarFirst`: by distance to the camera, far first; `NewestFirst`: reversed; the others as they are).
fn close_group(out: &mut DrawList, s: &System, camera: &Camera, p: GroupOpen) {
    use crate::def::SortMode;
    let [q0, r0, m0, d0] = p.marks;
    match p.sort {
        SortMode::FarFirst => {
            out.quads[q0..].sort_by(|a, b| b.center.distance_squared(camera.position).total_cmp(&a.center.distance_squared(camera.position)));
            out.meshes[m0..].sort_by(|a, b| Vec3::from(b.transform.translation).distance_squared(camera.position).total_cmp(&Vec3::from(a.transform.translation).distance_squared(camera.position)));
        }
        SortMode::NewestFirst => {
            out.quads[q0..].reverse();
            out.meshes[m0..].reverse();
        }
        SortMode::None | SortMode::OldestFirst => {}
    }
    let origin = s.xform.position;
    for (kind, from, to) in [(0u8, q0, out.quads.len()), (1, r0, out.ribbons.len()), (2, m0, out.meshes.len()), (3, d0, out.decals.len())] {
        if to > from {
            out.groups.push(crate::draw::DrawGroup { kind, from: from as u32, to: to as u32, system: s.id.0, emitter: p.emitter, priority: s.def.sort_priority, origin, offset: s.def.sort_offset });
        }
    }
}
