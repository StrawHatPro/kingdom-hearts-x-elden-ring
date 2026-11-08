//! The spell actors and shotlock projectiles: `ATresProjectileBase` with its movement and
//! attack-collision components as docs/sora-magic-spec.md parts B (generic life cycle, homing,
//! shapes), G 3 (the Kingdom Key shotlock projectiles, the Ragnarok helix) and I (what part B
//! left open: one hit per enemy, attack-row selection, Thunder, the Aero swirl counter and its
//! residue, the Water barrier, the Blizzard rail, Cure) give them. Engine-free plain data,
//! stepped once per core step with the REAL step time (projectiles are object type 5: never
//! slowed, F A6).
//!
//! Every spell is data: a [`StageDef`] is one blueprint of the sheet `sora-magic-projectiles`
//! (`<spell>.<stage>_<property>`: motion, homing, hit shapes, the respawn chain, the attack
//! rows); the class-specific code is what the game's native subclasses add: the Blizzard rail,
//! the Water bolt's steering, the Aero hit counter and residue, the Cure heal, the Ragnarok
//! helix.
//!
//! Order of one projectile's tick (B 1; the order of the attack-collision update against the
//! movement is not known: movement first, then the shapes swept from their previous position):
//! pending level impact -> life counters -> put effects (rail) -> life over? -> class tick ->
//! movement (move-start delay, homing, velocity, root sweep) -> attack shapes -> hits.
//!
//! Simplifications, each marked where it is:
//! - shape sweeps are sampled (4 steps), as in `combat/hit.rs`; a CONVEX cylinder is tested as
//!   a capsule of its radius around its axis;
//! - a deflected bolt (level hit shallower than the normal check) continues along the surface
//!   from the contact point with its speed kept; the stock deflection iterates further;
//! - the Blizzard rail (I 4.1): made are the wall test at the fire (`DoFire`: "closed"), the
//!   radius-25 sweep from the last put (a hit closes the rail there), the floor trace, the
//!   first node's radius-60 start test, the 10 cm rule and the capsule sweep between nodes; NOT
//!   made is the obstacle test between two nodes (radius-60 sweeps that move the floor point
//!   "to the ground under the obstacle": 0x1414757F0, read only as far as its structure);
//! - hit results 3..5 (a victim reflecting the projectile, B 1 step 6) do not exist in the seam.

use glam::Vec3;

use super::spells::{attack_row, AttackRow};
use super::tables::Sheet;
use super::{AeroPad, RailNode};
use crate::combat::hit::ShotStick; // (SHOT-FIX) F-POLISH's attach-to-actor record, shared with the form shots
use crate::enums::{ETresAbilityKind, ETresDamageAttribute, ETresDamageKind};
use crate::host::{Attach, EffectHandle, EffectId, EffectSpawn, HitInfo, HitRecord, HurtShape, ReactionResult, SoundId, SoundPlay, TargetFlags, TargetId, TargetInfo};
use crate::pawn::Ctx;
use crate::status::{compute_damage, DamageAttack, DamageContext, Defender};

/// The id of effects and sounds that are named, not numbered.
const NO_ID: u32 = u32::MAX;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShapeKind {
    /// Radius `size.x`.
    Sphere,
    /// A segment along the shape's Z of half length `size.z`, radius `size.x`.
    Capsule1,
    /// `Collision/tres_colltest01` x scale: a cylinder of radius 100, z -50..+50.
    Convex,
}

/// One shape of `TresAtkColl0.CollisionShapesSrc` (B 2.1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShapeDef {
    pub kind: ShapeKind,
    /// `m_DefaultAttackDataIDName`.
    pub row: &'static str,
    pub size: Vec3,
    pub inc_size: Vec3,
    pub inc_max_size: Vec3,
    pub scale: Vec3,
    pub inc_scale: Vec3,
    pub inc_max_scale: Vec3,
    /// World-axis offset from the actor.
    pub offset: Vec3,
    /// `m_SizeVectorCurve` on X: (t0, v0, t1, v1), linear, clamped outside.
    pub curve: Option<[f32; 4]>,
}

impl ShapeDef {
    /// (radius, half length along Z) at `t` seconds after activation (0x140649E40, 0x140649BF0).
    pub fn extent(&self, t: f32) -> (f32, f32) {
        let grow = |base: f32, inc: f32, max: f32| {
            let v = (base + inc * t).max(0.0);
            if max > 0.0 { v.min(max) } else { v }
        };
        let scale = |base: f32, inc: f32, max: f32| {
            let v = (base + inc * t).max(0.01);
            if max > 0.0 { v.min(max) } else { v }
        };
        let (sx, sy, sz) = (scale(self.scale.x, self.inc_scale.x, self.inc_max_scale.x), scale(self.scale.y, self.inc_scale.y, self.inc_max_scale.y), scale(self.scale.z, self.inc_scale.z, self.inc_max_scale.z));
        let mut size_x = grow(self.size.x, self.inc_size.x, self.inc_max_size.x);
        if let Some([t0, v0, t1, v1]) = self.curve {
            size_x = if t <= t0 {
                v0
            } else if t >= t1 || t1 <= t0 {
                v1
            } else {
                v0 + (v1 - v0) * (t - t0) / (t1 - t0)
            };
        }
        let size_z = grow(self.size.z, self.inc_size.z, self.inc_max_size.z);
        match self.kind {
            // (party stream D6b) The shape update 0x140647950 cases 0 / 1 / 3 / 4 / 5 (SPHERE, CAPSULE1 ...)
            // multiply the size by the smallest scale of the PARENT (the actor: 1) and reset the shape's
            // scale to 1; only CONVEX / DCONVEX (cases 6 / 8) keep the m_Scale growth (FUN_140649BF0) [C].
            // (Every SPHERE / CAPSULE1 of sora-magic-projectiles has scale 1 and no growth: no hit size changes.)
            ShapeKind::Sphere => (size_x, 0.0),
            ShapeKind::Capsule1 => (size_x, size_z),
            ShapeKind::Convex => (100.0 * sx.min(sy), 50.0 * sz),
        }
    }
}

/// `m_RespawnCheckType` (I 6.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RespawnCheck {
    HitAlways,
    HitAny,
    HitPawn,
    HitMap,
    NoHit,
}

/// `m_ReSpawnRotInheritType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RotInherit {
    Normal,
    YawOnly,
    Clear,
}

/// What the game's native subclass of the stage adds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProjClass {
    Generic,
    /// `TresProj_MagicFire`: the ground scorch (cosmetic).
    Fire,
    /// `TresProj_MagicBlizzard`: the ice rail.
    Blizzard,
    /// The Water barrier: attached to the caster.
    WaterBarrier,
    /// `TresProj_MagicWater` bolt steering.
    WaterBolt,
    /// `TresProj_MagicAero`: the swirl counter and the residue.
    Aero,
    /// `TresProj_MagicCure`: the heal.
    Cure,
    /// `TresProjKC_Shot`.
    KcShot,
    /// `TresProj_Ragnarok`: the helix.
    Ragnarok,
}

/// One blueprint of the projectile sheet: the class defaults the life cycle reads.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StageDef {
    /// The spell key ("fira") and the stage prefix ("", "burn_", "barrier_", "bolt_").
    pub family: &'static str,
    pub prefix: &'static str,
    pub class_name: &'static str,
    pub class: ProjClass,
    pub life: f32,
    pub max_distance: f32,
    pub no_destroy_on_hit: bool,
    pub explode_on_time_out: bool,
    pub stuck_time: f32,
    /// `m_OverrideFinishAttackDataID` / `m_OverrideSituationAttackDataID` ("None" = keep).
    pub finish_row: &'static str,
    pub situation_row: &'static str,
    pub put_effect: bool,
    pub put_duration: f32,
    pub put_first: f32,
    pub put_xy: bool,
    /// `m_bReSpawnProjectileOnTimeOut`: the master enable of the successor.
    pub respawn_enabled: bool,
    pub respawn_check: RespawnCheck,
    pub respawn_class: &'static str,
    pub respawn_rot: RotInherit,
    pub take_target: bool,
    pub take_hit_list: bool,
    pub take_finish: bool,
    pub initial_speed: f32,
    pub max_speed: f32,
    pub accel: f32,
    pub move_start_delay: f32,
    pub normal_check: bool,
    pub normal_angle: f32,
    pub homing: bool,
    pub homing_delay: f32,
    pub homing_min_dist: f32,
    pub turn_max: f32,
    pub turn_accel: f32,
    pub max_turn_max: f32,
    pub homing_max_angle: f32,
    /// Radius of the root sphere that is swept against the level; 0 = a static actor.
    pub root_radius: f32,
    pub shapes: [Option<ShapeDef>; 2],
    /// Water bolt: `m_bSetFallingIfNoHoming`.
    pub set_falling: bool,
    /// Aero: `m_SwirlLimit` (-1 = none).
    pub swirl_limit: i32,
    /// Blizzard: `m_GenerateRailHeight`, `m_GenerateRailLife`.
    pub rail_height: f32,
    pub rail_life: f32,
    /// Aero: the residue class exists.
    pub residue: bool,
    /// Cure: `m_DetectRange`, `m_AppendCure`, `m_BonusAppendCure`.
    pub cure: Option<(f32, i32, i32)>,
    /// Effects / sounds of a hit on a pawn and on the map (the shotlock blueprints name them).
    pub hit_effect: &'static str,
    pub hit_sound: &'static str,
    pub map_hit_effect: &'static str,
    /// (SHOT-A) The sheet the blueprint is a row of: `Sheet::Proj` (spells, `<family>.<prefix><property>`) or
    /// `Sheet::Shotlocks` (the ShootFlow classes, `proj.<family>.<property>`); the successor is looked up there.
    pub sheet: Sheet,
    /// (SHOT-A) `TresProj_Ragnarok`'s helix values (its native ctor 0x1414700D0, or the blueprint's overrides).
    pub rolling: Rolling,
    /// (SHOT-FIX) `m_bAttachToHitActor` (+0x5F0 bit 3): Explode's slot 537 (0x141473E30) attaches the actor to what it
    /// hit (KeepWorldTransform; `m_AttachToHitActorOffset` +0x5F4 is set by no projectile blueprint: 0); it then rides on
    /// it for its `m_ExplodeLife`.
    pub attach_to_hit_actor: bool,
    /// (SHOT-FIX) `m_ExplodeLife`: the seconds the actor lives on after Shutdown (0x141475670 copies it to +0x758; the tick
    /// 0x141484B10 counts it down and destroys the actor at <= 0, the tick after for 0: native 0).
    pub explode_life: f32,
    /// (SHOT-FIX) `m_bDoDelayFire`, `m_FireDelayTimeMin`, `m_FireDelayTimeMax`: StartFire 0x141474970 waits that long (a
    /// random time between them when min < max) before DoFire, the actor's collision off meanwhile.
    pub delay_fire: (bool, f32, f32),
    /// (HM-MAGIC) `m_CollAutoActivate[0].m_AttackInterval`: with an interval > 0 the hit list is emptied every interval from
    /// the first hit (TresAtkColl TimerTick 0x14062CFC0, as `combat/hit.rs`'s windows). Only the turn-type
    /// `TresProj_Mag*CL` actors (magic/hm_set.rs) have one; 0 for every other blueprint (each actor hits an enemy once).
    pub hit_interval: f32,
}

/// (SHOT-A) The helix of a `TresProj_Ragnarok` subclass (magic spec G 3.3): `m_RollingSpeed` / `Add` / `Max`,
/// `m_RollingRadius` / `Add` / `Max`, the opening rate of the radius (Tick 0x141484643).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rolling {
    pub speed: f32,
    pub speed_add: f32,
    pub speed_max: f32,
    pub radius: f32,
    pub radius_add: f32,
    pub radius_max: f32,
    pub open_rate: f32,
}

impl Rolling {
    /// The Kingdom Key Ragnarok's (`kc2.proj_kc_ragnarok.rolling_*`: the native defaults).
    pub fn kc() -> Rolling {
        let s = Sheet::Shot;
        let r = |p: &str| s.num(&format!("kc2.proj_kc_ragnarok.rolling_{p}"));
        Rolling { speed: r("speed"), speed_add: r("speed_add"), speed_max: r("speed_max"), radius: r("radius"), radius_add: r("radius_add"), radius_max: r("radius_max"), open_rate: r("radius_open_rate") }
    }

    /// A ShootFlow class's (`proj.<key>.rolling_*` of sora-shotlocks), the Kingdom Key's values where a row is missing.
    fn shot(key: &str) -> Rolling {
        let kc = Rolling::kc();
        let s = Sheet::Shotlocks;
        let r = |p: &str, d: f32| s.num_or(&format!("proj.{key}.rolling_{p}"), d);
        Rolling { speed: r("speed", kc.speed), speed_add: r("speed_add", kc.speed_add), speed_max: r("speed_max", kc.speed_max), radius: r("radius", kc.radius), radius_add: r("radius_add", kc.radius_add), radius_max: r("radius_max", kc.radius_max), open_rate: r("radius_open_rate", kc.open_rate) }
    }
}

const STAGES: [&str; 4] = ["", "burn_", "barrier_", "bolt_"];

impl StageDef {
    /// The stage `prefix` of the spell `family`; `None` when the sheet has no such stage.
    pub fn load(family: &'static str, prefix: &'static str) -> Option<StageDef> {
        Self::load_in(Sheet::Proj, family, prefix)
    }

    /// (SHOT-A) A ShootFlow projectile class of sora-shotlocks by its key ("he_drain" = `TresProjHE_Drain_C`).
    pub fn shot(key: &'static str) -> Option<StageDef> {
        Self::load_in(Sheet::Shotlocks, key, "")
    }

    /// (SHOT-A) A ShootFlow projectile by its blueprint class ("TresProjHE_DrainEx_C"): the `proj.<key>.class` row.
    pub fn shot_class(class_name: &str) -> Option<StageDef> {
        let id = Sheet::Shotlocks.rows().iter().find(|(id, v)| id.starts_with("proj.") && id.ends_with(".class") && matches!(v, super::tables::PV::S(c) if *c == class_name))?.0;
        Self::shot(&id["proj.".len()..id.len() - ".class".len()])
    }

    /// (SHOT-A) `load` in a given sheet: `Sheet::Proj` rows are `<family>.<prefix><property>`, `Sheet::Shotlocks`
    /// rows `proj.<family>.<property>` (the prefix is unused there); the class comes from `native_class` in the latter.
    pub fn load_in(sheet: Sheet, family: &'static str, prefix: &'static str) -> Option<StageDef> {
        let s = sheet;
        let shot = sheet == Sheet::Shotlocks;
        let id = |property: &str| if shot { format!("proj.{family}.{property}") } else { format!("{family}.{prefix}{property}") };
        let class_name = match s.find(&id("class")) {
            Some(super::tables::PV::S(name)) => name,
            _ => return None,
        };
        let shape = |k: usize| -> Option<ShapeDef> {
            if (s.num_or(&id("hit_shape_count"), 0.0) as usize) <= k {
                return None;
            }
            let sid = |property: &str| id(&format!("shape{k}_{property}"));
            let kind = match s.text(&sid("type")) {
                "SPHERE" => ShapeKind::Sphere,
                "CAPSULE1" => ShapeKind::Capsule1,
                _ => ShapeKind::Convex,
            };
            let curve = match s.find(&sid("size_curve_x_keys")) {
                Some(super::tables::PV::A(&[t0, v0, t1, v1])) => Some([t0, v0, t1, v1]),
                _ => None,
            };
            Some(ShapeDef { kind, row: s.text(&sid("attack_id")), size: s.vec3(&sid("size")), inc_size: s.vec3(&sid("inc_size")), inc_max_size: s.vec3(&sid("inc_max_size")), scale: s.vec3(&sid("scale")), inc_scale: s.vec3(&sid("inc_scale")), inc_max_scale: s.vec3(&sid("inc_max_scale")), offset: s.vec3(&sid("offset")), curve })
        };
        let class = if shot {
            // (SHOT-A) the native class decides (TresProj_Ragnarok: the helix); the others are TresProjectileBase.
            if s.text_or(&id("native_class"), "") == "TresProj_Ragnarok" { ProjClass::Ragnarok } else { ProjClass::Generic }
        } else if family.starts_with("fi") {
            ProjClass::Fire
        } else if family.starts_with("blizza") && prefix.is_empty() {
            ProjClass::Blizzard
        } else if family.starts_with("wat") && prefix == "barrier_" {
            ProjClass::WaterBarrier
        } else if family.starts_with("wat") && prefix == "bolt_" {
            ProjClass::WaterBolt
        } else if family.starts_with("aero") && family != "aero_cylinder" {
            ProjClass::Aero
        } else if family.starts_with("cur") {
            ProjClass::Cure
        } else {
            ProjClass::Generic
        };
        let speed = s.num_or(&id("initial_speed"), 0.0);
        let respawn_check = match s.text_or(&id("respawn_check_type"), "TPR_TYPE_HIT_ALWAYS") {
            "TPR_TYPE_HIT_ANY" => RespawnCheck::HitAny,
            "TPR_TYPE_HIT_PAWN" => RespawnCheck::HitPawn,
            "TPR_TYPE_HIT_MAP" => RespawnCheck::HitMap,
            "TPR_TYPE_NO_HIT" => RespawnCheck::NoHit,
            _ => RespawnCheck::HitAlways,
        };
        let respawn_rot = match s.text_or(&id("re_spawn_rot_inherit_type"), "TPRR_NORMAL") {
            "TPRR_YAW_ONLY" => RotInherit::YawOnly,
            "TPRR_CLEAR" => RotInherit::Clear,
            _ => RotInherit::Normal,
        };
        let cure = (class == ProjClass::Cure).then(|| (s.num(&id("detect_range")), s.int(&id("append_cure")), s.int(&id("bonus_append_cure"))));
        Some(StageDef {
            family,
            prefix,
            class_name,
            class,
            life: s.num_or(&id("main_life"), 0.0),
            max_distance: s.num_or(&id("max_life_distance"), 0.0),
            no_destroy_on_hit: s.flag_or(&id("no_destroy_on_hit"), false),
            explode_on_time_out: s.flag_or(&id("explode_on_time_out"), false),
            stuck_time: s.num_or(&id("auto_stuck_shutdown_time"), 0.0),
            finish_row: s.text_or(&id("override_finish_attack_data_id"), "None"),
            situation_row: s.text_or(&id("override_situation_attack_data_id"), "None"),
            put_effect: s.flag_or(&id("call_put_effect_event_call"), false),
            put_duration: s.num_or(&id("put_effect_duration"), 0.0),
            put_first: s.num_or(&id("first_put_effect_duration"), 0.0),
            put_xy: s.flag_or(&id("call_put_effect_dist_check_xy"), false),
            respawn_enabled: s.flag_or(&id("re_spawn_projectile_on_time_out"), false),
            respawn_check,
            respawn_class: s.text_or(&id("re_spawn_projectile_class"), "null"),
            respawn_rot,
            take_target: s.flag_or(&id("re_spawn_take_over_atk_target"), false),
            take_hit_list: s.flag_or(&id("re_spawn_take_over_atk_coll_hit_list"), false),
            take_finish: s.flag_or(&id("re_spawn_take_over_atk_coll_finish_flag"), false),
            initial_speed: speed,
            max_speed: s.num_or(&id("max_speed"), speed),
            accel: s.num_or(&id("accel"), 0.0),
            move_start_delay: s.num_or(&id("move_start_delay"), 0.0),
            normal_check: s.flag_or(&id("enable_map_hit_normal_check"), false),
            normal_angle: s.num_or(&id("check_map_hit_normal"), 90.0),
            homing: s.flag_or(&id("enable_homing"), false),
            homing_delay: s.num_or(&id("homing_start_delay"), 0.0),
            homing_min_dist: s.num_or(&id("homing_min_dist"), 0.0),
            turn_max: s.num_or(&id("homing_turn_max"), 0.0),
            turn_accel: s.num_or(&id("homing_turn_accel"), 0.0),
            max_turn_max: s.num_or(&id("homing_max_turn_max"), 0.0),
            homing_max_angle: s.num_or(&id("homing_max_angle"), 0.0),
            // A static actor makes no root sweep (the sheet's null rows: see the registry).
            root_radius: if speed > 0.0 { s.num_or(&id("root_radius"), 25.0) } else { 0.0 },
            shapes: [shape(0), shape(1)],
            set_falling: s.flag_or(&id("set_falling_if_no_homing"), false),
            swirl_limit: s.num_or(&id("swirl_limit"), -1.0) as i32,
            rail_height: s.num_or(&id("generate_rail_height"), 0.0),
            rail_life: if class == ProjClass::Blizzard { s.num_or(&id("generate_rail_life"), 0.0) } else { 0.0 },
            residue: s.find(&id("residue_class")).is_some(),
            cure,
            // (SHOT-A) the ShootFlow classes name their hit effects; the spells' rows carry none.
            hit_effect: if shot { s.text_or(&id("hit_effect"), "") } else { "" },
            hit_sound: if shot { s.text_or(&id("hit_sound"), "") } else { "" },
            map_hit_effect: if shot { s.text_or(&id("map_hit_effect"), "") } else { "" },
            sheet,
            rolling: if shot && class == ProjClass::Ragnarok { Rolling::shot(family) } else { Rolling::kc() },
            // (SHOT-FIX) the ShootFlow classes' rows (tools/sora_shotlocks.js); the spells' rows carry none (native false / 0)
            attach_to_hit_actor: s.flag_or(&id("attach_to_hit_actor"), false),
            explode_life: s.num_or(&id("explode_life"), 0.0),
            delay_fire: (s.flag_or(&id("do_delay_fire"), false), s.num_or(&id("fire_delay_time_min"), 0.0), s.num_or(&id("fire_delay_time_max"), 0.0)),
            hit_interval: 0.0, // (HM-MAGIC) the spells' and shotlocks' blueprints: hit once per actor (magic spec I 0.2)
        })
    }

    /// The first actor a cast of `family` spawns: the barrier for Water, else the main stage.
    pub fn first(family: &'static str) -> Option<StageDef> {
        StageDef::load(family, "barrier_").or_else(|| StageDef::load(family, ""))
    }

    /// The successor blueprint (`m_ReSpawnProjectileClass`): the stage of the same spell with
    /// that class.
    pub fn successor(&self) -> Option<StageDef> {
        if self.respawn_class == "null" || self.respawn_class == "None" {
            return None;
        }
        if self.sheet == Sheet::Shotlocks {
            return StageDef::shot_class(self.respawn_class); // (SHOT-A)
        }
        STAGES.iter().filter_map(|prefix| StageDef::load(self.family, prefix)).find(|stage| stage.class_name == self.respawn_class)
    }

    /// `TresProjKC_Shot` / `TresProjKC_Ragnarok` (G 3.2, 3.3).
    pub fn shotlock(ragnarok: bool) -> StageDef {
        let s = Sheet::Shot;
        let group = if ragnarok { "kc2.proj_kc_ragnarok" } else { "kc2.proj_kc_shot" };
        let id = |property: &str| format!("{group}.{property}");
        let row = s.text(&id("atk_row"));
        let radius = s.num(&id("atk_radius"));
        let sphere = ShapeDef { kind: ShapeKind::Sphere, row, size: Vec3::new(radius, 0.0, 0.0), inc_size: Vec3::ZERO, inc_max_size: Vec3::ZERO, scale: Vec3::ONE, inc_scale: Vec3::ZERO, inc_max_scale: Vec3::ZERO, offset: Vec3::ZERO, curve: None };
        StageDef {
            family: if ragnarok { "kc_ragnarok" } else { "kc_shot" },
            prefix: "",
            class_name: if ragnarok { "TresProjKC_Ragnarok_C" } else { "TresProjKC_Shot_C" },
            class: if ragnarok { ProjClass::Ragnarok } else { ProjClass::KcShot },
            life: s.num(&id("main_life")),
            max_distance: 0.0,
            no_destroy_on_hit: false,
            explode_on_time_out: false,
            stuck_time: 0.0,
            finish_row: "None",
            situation_row: "None",
            put_effect: false,
            put_duration: 0.0,
            put_first: 0.0,
            put_xy: false,
            respawn_enabled: false,
            respawn_check: RespawnCheck::HitAlways,
            respawn_class: "null",
            respawn_rot: RotInherit::Normal,
            take_target: false,
            take_hit_list: false,
            take_finish: false,
            initial_speed: s.num(&id("initial_speed")),
            max_speed: s.num(&id("max_speed")),
            accel: s.num(&id("accel")),
            move_start_delay: s.num(&id("move_start_delay")),
            // `m_bEnableMapHitNormalCheck` false (native): every blocking hit stops it.
            normal_check: false,
            normal_angle: 90.0,
            homing: true,
            homing_delay: s.num(&id("homing_start_delay")),
            homing_min_dist: s.num(&id("homing_min_dist")),
            turn_max: s.num(&id("homing_turn_max")),
            turn_accel: s.num(&id("homing_turn_accel")),
            max_turn_max: s.num_or(&id("homing_max_turn_max"), 0.0),
            homing_max_angle: 0.0,
            root_radius: s.num(&id("root_radius")),
            shapes: [Some(sphere), None],
            set_falling: false,
            swirl_limit: -1,
            rail_height: 0.0,
            rail_life: 0.0,
            residue: false,
            cure: None,
            hit_effect: if ragnarok { "vw_so010_rgn_hit00" } else { "vw_sor_rbs_hit0" },
            hit_sound: if ragnarok { "" } else { "se02001_020" },
            map_hit_effect: if ragnarok { "vw_so010_rgn_wht00" } else { "vw_sor_rbs_wht0" },
            sheet: Sheet::Shot, // (SHOT-A)
            rolling: Rolling::kc(), // (SHOT-A)
            // (SHOT-FIX) TresProjKC_Shot / _Ragnarok set none of these (native: false, 0)
            attach_to_hit_actor: false,
            explode_life: 0.0,
            delay_fire: (false, 0.0, 0.0),
            hit_interval: 0.0, // (HM-MAGIC)
        }
    }
}

impl StageDef {
    /// `ATHLETIC_KICK` = `Blueprints/Projectile/TresProj_AthleticKick` (magic spec H 4.7): no
    /// movement, life 0.5 s, not destroyed on hit, one attack sphere of radius 100 growing 800 /s
    /// to 350, row `FF_ATHLETIC_KICK` (power 1.0, BLOW, physical).
    pub fn athletic_kick() -> StageDef {
        let s = Sheet::Shot;
        let sphere = ShapeDef { kind: ShapeKind::Sphere, row: "FF_ATHLETIC_KICK", size: Vec3::new(s.num("airstep2.kick_proj.radius"), 0.0, 0.0), inc_size: Vec3::new(s.num("airstep2.kick_proj.radius_growth"), 0.0, 0.0), inc_max_size: Vec3::new(s.num("airstep2.kick_proj.radius_max"), 0.0, 0.0), scale: Vec3::ONE, inc_scale: Vec3::ZERO, inc_max_scale: Vec3::ZERO, offset: Vec3::ZERO, curve: None };
        StageDef {
            family: "athletic_kick",
            class_name: "TresProj_AthleticKick_C",
            class: ProjClass::Generic,
            life: s.num("airstep2.kick_proj.life"),
            no_destroy_on_hit: true,
            initial_speed: 0.0,
            max_speed: 0.0,
            accel: 0.0,
            move_start_delay: 0.0,
            homing: false,
            root_radius: 0.0,
            shapes: [Some(sphere), None],
            hit_effect: "",
            hit_sound: "",
            map_hit_effect: "",
            ..StageDef::shotlock(false)
        }
    }
}

/// What a projectile ran into.
#[derive(Clone, Copy, Debug, PartialEq)]
enum HitWhat {
    /// Level geometry: (point, impact normal).
    Level(Vec3, Vec3),
    Pawn(TargetId),
}

/// One live actor.
#[derive(Clone, Debug, PartialEq)]
pub struct Projectile {
    pub id: u32,
    pub def: StageDef,
    pub position: Vec3,
    pub velocity: Vec3,
    /// The actor's forward axis (rotation follows velocity).
    pub forward: Vec3,
    /// `m_CommandKind` (the cast command; 0xFD for a shotlock) and `m_ShootFlowKind`.
    pub cmd: u8,
    pub shoot_flow: u8,
    /// `SetFinish(bFinish, bSituation)` (I 0.3).
    pub finish: bool,
    pub situation: bool,
    /// `m_bIsEnableFullMpBurst`.
    pub full_mp: bool,
    /// The combo values of the cast: (index, A, A at Begin, B at Begin) = info +0x52..+0x55.
    pub combo: [i32; 4],
    /// The homing target; the flag "a target is set" (+0x34C bit 0).
    pub target: TargetId,
    pub has_target: bool,
    homing_on: bool,
    homing_delay: f32,
    turn_max: f32,
    /// `m_fMoveStartDelay` left; the movement component's "active" and "started" bits.
    pub move_delay: f32,
    pub active: bool,
    pub started: bool,
    /// Time since StartFire / distance travelled.
    pub age: f32,
    pub distance: f32,
    last_position: Vec3,
    fire_position: Vec3,
    stuck_timer: f32,
    grazing: bool,
    /// A level impact of this tick, processed on the next.
    pending_impact: Option<(Vec3, Vec3)>,
    /// Targets already hit by this actor or its predecessors.
    pub hit_list: Vec<TargetId>,
    shape_previous: [Option<Vec3>; 2],
    /// Distance of the next put effect; the last put position.
    next_put: f32,
    put_origin: Vec3,
    /// Rail: the node laid last, and "closed".
    rail_previous: Option<(u32, Vec3)>,
    rail_closed: bool,
    /// +0x8B4: the position of the last put.
    rail_last_put: Vec3,
    /// The barrier rides on Sora: its offset in his frame.
    attached: Option<Vec3>,
    /// Aero: hits counted, the limit still in force, the kind later hits get.
    swirl_count: i32,
    swirl_limit: i32,
    after_kind: Option<ETresDamageKind>,
    residue_spawned: bool,
    /// Water bolt: the one-time turn rate by distance was set.
    turn_set: bool,
    /// Ragnarok: helix phase, angle, radius, their rates.
    helix_phase: u8,
    helix_angle: f32,
    helix_radius: f32,
    helix_radius_add: f32,
    helix_speed: f32,
    helix_speed_add: f32,
    /// Hits scored (for tests).
    pub hits: u32,
    /// The persistent effect that stands for the actor (stopped when it dies).
    pub effect: Option<EffectHandle>,
    pub dead: bool,
    /// (SHOT-FIX) Attached to a target actor (FUN_143E8FC60 with KeepWorldTransform: by its spawner, or by Explode's
    /// `m_bAttachToHitActor`): that actor and the actor's place in its frame; it moves with it (origin and yaw, as
    /// combat/hit.rs `ShotStick` does for the form shots). Generic: any target the host lists.
    pub stick: Option<ShotStick>,
    /// (SHOT-FIX) StartFire's wait (+0x6F4 bit 0 / +0x6F8): DoFire when it runs out (0x141484B10).
    pub fire_delay: Option<f32>,
    /// (SHOT-FIX) SetDelayFire's values (+0x5D0 bit 0 `m_bDoDelayFire`, `m_FireDelayTimeMin` / `Max`), read by StartFire.
    delay_fire: (bool, f32, f32),
    /// (SHOT-FIX) Exploded and attached: the `m_ExplodeLife` left (+0x758) before the actor is destroyed.
    linger: Option<f32>,
    /// (HM-MAGIC) TimerTick's clock of the hit list (rec+8), for a blueprint with a hit interval.
    hit_timer: f32,
}

/// A projectile as a host draws it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProjectileView {
    pub id: u32,
    /// The blueprint class ("TresProj_MagFiraNml_C").
    pub class: &'static str,
    pub position: Vec3,
    pub forward: Vec3,
    /// The first hit shape now: (centre, radius, half length along Z); radius 0 = none.
    pub shape: (Vec3, f32, f32),
    pub effect: Option<EffectHandle>,
    pub age: f32,
}

/// What the spawner sets on a new actor (the setters of A 1.4 step 8 and G 3.4).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpawnArgs {
    pub def: StageDef,
    pub position: Vec3,
    /// Unit direction (rotation of the actor; `Velocity = dir * InitialSpeed`).
    pub direction: Vec3,
    pub cmd: u8,
    pub shoot_flow: u8,
    pub finish: bool,
    pub situation: bool,
    pub full_mp: bool,
    pub combo: [i32; 4],
    pub target: TargetId,
    /// Relative offset when the actor is attached to Sora's root (the Water barrier).
    pub attached: Option<Vec3>,
    /// `StartFire` at once (false: the shotlock formation fires it itself).
    pub fire: bool,
}

impl Projectile {
    pub fn new(id: u32, args: &SpawnArgs) -> Projectile {
        let def = args.def;
        let direction = if args.direction.length_squared() > 1.0e-8 { args.direction.normalize() } else { Vec3::ZERO };
        let ragnarok = def.class == ProjClass::Ragnarok;
        Projectile {
            id,
            def,
            position: args.position,
            velocity: direction * def.initial_speed,
            forward: if direction == Vec3::ZERO { Vec3::X } else { direction },
            cmd: args.cmd,
            shoot_flow: args.shoot_flow,
            finish: args.finish,
            situation: args.situation,
            full_mp: args.full_mp,
            combo: args.combo,
            target: args.target,
            has_target: args.target != TargetId::NONE,
            homing_on: def.homing,
            homing_delay: def.homing_delay,
            turn_max: def.turn_max,
            move_delay: def.move_start_delay,
            // The setter 0x141482960: `active = false; delay = d; if d <= 0: active = true`.
            active: def.move_start_delay <= 0.0,
            started: args.fire,
            age: 0.0,
            distance: 0.0,
            last_position: args.position,
            fire_position: args.position,
            stuck_timer: 0.0,
            grazing: false,
            pending_impact: None,
            hit_list: Vec::new(),
            shape_previous: [None, None],
            next_put: if def.put_first > 0.0 { def.put_first } else { 0.0 },
            put_origin: args.position,
            rail_previous: None,
            rail_closed: false,
            rail_last_put: args.position,
            attached: args.attached,
            swirl_count: 0,
            swirl_limit: def.swirl_limit,
            after_kind: None,
            residue_spawned: false,
            turn_set: false,
            helix_phase: 0,
            helix_angle: 0.0,
            helix_radius: 0.0,
            // (SHOT-A) the class's own helix values (the Kingdom Key's: kc2.proj_kc_ragnarok.rolling_*)
            helix_radius_add: if ragnarok { def.rolling.radius_add } else { 0.0 },
            helix_speed: if ragnarok { def.rolling.speed } else { 0.0 },
            helix_speed_add: if ragnarok { def.rolling.speed_add } else { 0.0 },
            hits: 0,
            effect: None,
            dead: false,
            // (SHOT-FIX) not attached, not waiting, the class's delayed-fire defaults until SetDelayFire
            stick: None,
            fire_delay: None,
            delay_fire: def.delay_fire,
            linger: None,
            hit_timer: 0.0, // (HM-MAGIC)
        }
    }

    /// (SHOT-FIX) `SetDelayFire(on, min, max)` 0x141482540: only before the actor was fired (+0x6C0 bit 0).
    pub fn set_delay_fire(&mut self, on: bool, min: f32, max: f32) {
        if !self.started {
            self.delay_fire = (on, min, max);
        }
    }

    /// (SHOT-FIX) The actor attached to `target` where it is now (FUN_143E8FC60(actor, the target's component,
    /// KeepWorldTransform DAT_148750ED0)): its place in the target's frame is kept from then on.
    pub fn attach_to(&mut self, target: &TargetInfo) {
        self.stick = Some(ShotStick { target: target.id, local: rotate_yaw(self.position - target.position, -target.yaw), bone: None });
    }

    /// (AUDIT-B W-FIN) Slot 537 (0x141473E30, m_bAttachToHitActor) at a hit on `target`: attached at the hit's bone
    /// (FHitResult.BoneName, KeepWorldTransform) when the host has bones (combat/hit.rs `ShotStick::at_hit`), else as
    /// `attach_to`.
    pub fn attach_to_hit(&mut self, targets: &dyn crate::host::Targets, target: &TargetInfo) {
        self.stick = Some(ShotStick::at_hit(targets, target, self.position, 0.0));
    }

    pub fn view(&self) -> ProjectileView {
        let shape = match self.def.shapes[0] {
            Some(shape) => {
                let (radius, half) = shape.extent(self.age);
                (self.position + shape.offset, radius, half)
            }
            None => (self.position, 0.0, 0.0),
        };
        ProjectileView { id: self.id, class: self.def.class_name, position: self.position, forward: self.forward, shape, effect: self.effect, age: self.age }
    }

    /// `SetDirection(dir)` (slot 577): `Velocity = dir * InitialSpeed`; the rotation follows.
    pub fn set_direction(&mut self, direction: Vec3) {
        if direction.length_squared() > 1.0e-8 {
            self.forward = direction.normalize();
            self.velocity = self.forward * self.def.initial_speed;
        }
    }

    /// `SetMoveStartDelay(d)` (slot 584 -> component 0x141482960).
    pub fn set_move_start_delay(&mut self, delay: f32) {
        self.active = false;
        self.move_delay = delay;
        if delay <= 0.0 {
            self.active = true;
        }
    }

    /// The attack row a shape uses now (I 0.3).
    fn row_name(&self, shape: &ShapeDef) -> &'static str {
        let none = |name: &str| name == "None" || name == "null" || name.is_empty();
        if self.situation && !none(self.def.situation_row) {
            self.def.situation_row
        } else if self.finish && !self.situation && !none(self.def.finish_row) {
            self.def.finish_row
        } else {
            shape.row
        }
    }
}

/// Spawns an actor (the projectile manager's `Spawn` + the setters + `StartFire`): returns its
/// id. The actor's persistent effect is started here (name = the blueprint class).
pub(crate) fn spawn(cx: &mut Ctx, args: &SpawnArgs) -> u32 {
    spawn_fired_from(cx, args, args.position)
}

/// (vfx implementer 4) The seam id of an actor's FIRE effects event (see `spawn_fired_from`).
pub const FIRE_EFFECTS_ID: u32 = 0xF17E_0001;

/// (vfx implementer 4) `spawn` with the fire position of the caster's call: the manager's Spawn
/// gets (firePos, pos) apart, and slot 575 SpawnFireEffects (0x141482ED0) plays every
/// `MyFireEffects` entry AT THE FIRE POSITION with the actor's rotation (magic spec B 1; A 1.4
/// step 8: `vt+0x11F8(&P)`, P = the weapon's muzzle). For Thunder and Aero the actor itself is at
/// the target (`m_bIsDirectPos`), so its fire effect (the cast flash, vm_thd_fir0 / vm_air_win0)
/// belongs on the Keyblade, not on the target. Two seam events: the class name with the actor
/// (persistent: its body effect) and the class name with `FIRE_EFFECTS_ID` at the fire position
/// (one-shot: its fire effects).
pub(crate) fn spawn_fired_from(cx: &mut Ctx, args: &SpawnArgs, fire_position: Vec3) -> u32 {
    let id = cx.pawn.magic.next_id();
    let mut projectile = Projectile::new(id, args);
    let at = match args.attached {
        Some(offset) => Attach::Sora { bone: None, offset },
        None => Attach::World(args.position),
    };
    projectile.effect = Some(cx.spawn_effect(EffectSpawn { id: EffectId(NO_ID), name: args.def.class_name, at, rotation: Vec3::ZERO, scale: 1.0, persistent: true }));
    {
        let d = projectile.forward;
        let rotation = Vec3::new(d.z.clamp(-1.0, 1.0).asin().to_degrees(), d.y.atan2(d.x).to_degrees(), 0.0);
        cx.spawn_effect(EffectSpawn { id: EffectId(FIRE_EFFECTS_ID), name: args.def.class_name, at: Attach::World(fire_position), rotation, scale: 1.0, persistent: false });
    }
    // Blizzard's `DoFire` (0x141474C80): a line from behind the spawn point (spawn - direction x
    // the horizontal distance caster -> spawn) to the spawn point; a blocking hit on level
    // geometry (actor category 0) sets "closed" (+0x8B0 bit 0): no rail for this bolt.
    if args.def.class == ProjClass::Blizzard && args.def.rail_life > 0.0 {
        let back = (args.position - cx.pawn.cmc.location).truncate().length();
        let dir = projectile.forward;
        if back > 0.0 && cx.world.line_trace(args.position - dir * back, args.position).is_some() {
            projectile.rail_closed = true;
        }
    }
    // (SHOT-FIX) `fire`: StartFire now, which waits first when the class delays its fire (m_bDoDelayFire).
    if args.fire {
        let delay = fire_delay_of(cx, projectile.delay_fire);
        projectile.started = delay.is_none();
        projectile.fire_delay = delay;
    }
    cx.pawn.magic.projectiles.push(projectile);
    id
}

/// (SHOT-FIX) StartFire 0x141474970's wait: with `m_bDoDelayFire` and a min or max above 0, min (a random time in
/// [min, max) when min < max: the global xorshift's 24 bits x 2^-24, here `PawnMagic::next_random`); else none (DoFire at
/// once, vt+0x1308).
fn fire_delay_of(cx: &mut Ctx, (on, min, max): (bool, f32, f32)) -> Option<f32> {
    if !on || (min <= 0.0 && max <= 0.0) {
        return None;
    }
    Some(if min < max { cx.pawn.magic.next_random() as f32 * 5.960_464_5e-8 * (max - min) + min } else { min })
}

/// (SHOT-FIX) `StartFire` (0x141474970) of an actor spawned unfired (`SpawnArgs::fire` false): DoFire now, or after its
/// delay (SetDelayFire); the actor's collision is off meanwhile (FUN_143EAC290(actor, 0): it neither moves nor hits).
pub(crate) fn start_fire(cx: &mut Ctx, id: u32) {
    let Some(delay_fire) = cx.pawn.magic.projectiles.iter().find(|p| p.id == id && !p.started).map(|p| p.delay_fire) else { return };
    let delay = fire_delay_of(cx, delay_fire);
    if let Some(p) = cx.pawn.magic.projectiles.iter_mut().find(|p| p.id == id) {
        p.started = delay.is_none();
        p.fire_delay = delay;
    }
}

/// (SHOT-FIX) An attached actor follows its target's origin and yaw (combat/hit.rs's rule for `ShotStick`); a target that
/// is gone leaves it where it was.
fn follow_stick(cx: &Ctx, p: &mut Projectile) {
    let Some(s) = p.stick.filter(ShotStick::stuck) else { return };
    // (AUDIT-B W-FIN) on the hit's bone when it was attached at one (`ShotStick::follow`), else the origin and yaw
    if let Some((at, _)) = s.follow(&*cx.host.targets) {
        p.position = at;
    }
}

fn rotate_yaw(v: Vec3, yaw_degrees: f32) -> Vec3 {
    let (sin, cos) = yaw_degrees.to_radians().sin_cos();
    Vec3::new(v.x * cos - v.y * sin, v.x * sin + v.y * cos, v.z)
}

/// One step of every projectile, rail node and pad. `dt` is the REAL step time.
pub(crate) fn tick(cx: &mut Ctx, dt: f32) {
    cx.pawn.magic.shut_down.clear(); // (SHOT-A cannon) this tick's shutdowns only
    let mut list = std::mem::take(&mut cx.pawn.magic.projectiles);
    let mut spawned: Vec<SpawnArgs> = Vec::new();
    let mut inherited: Vec<Vec<TargetId>> = Vec::new();
    for p in list.iter_mut() {
        // (SHOT-FIX) an attached actor rides on its target (the attachment moves it before its own tick)
        if !p.dead {
            follow_stick(cx, p);
        }
        // (SHOT-FIX) exploded and attached: it lives `m_ExplodeLife` more (0x141484B10's +0x758 branch), then it goes
        if let Some(left) = p.linger {
            let left = left - dt;
            p.linger = Some(left);
            if left <= 0.0 {
                p.dead = true;
            }
        } else if !p.dead && !p.started {
            // (SHOT-FIX) StartFire's wait (0x141484B10: +0x6F8 -= dt; at <= 0 collision on and DoFire, then this tick's
            // own update)
            if let Some(left) = p.fire_delay {
                let left = left - dt;
                p.fire_delay = Some(left);
                if left <= 0.0 {
                    p.fire_delay = None;
                    p.started = true;
                }
            }
        }
        if !p.dead && p.started && p.linger.is_none() {
            step(cx, p, dt, &mut spawned, &mut inherited);
        }
        if p.dead {
            if let Some(handle) = p.effect.take() {
                cx.stop_effect(handle, false);
            }
        }
    }
    list.retain(|p| !p.dead);
    // Projectiles the step itself added (none today) stay; then the successors, fired at once.
    list.append(&mut cx.pawn.magic.projectiles);
    cx.pawn.magic.projectiles = list;
    for (args, hit_list) in spawned.iter().zip(inherited) {
        let id = spawn(cx, args);
        if let Some(p) = cx.pawn.magic.projectiles.iter_mut().find(|p| p.id == id) {
            p.hit_list = hit_list;
        }
    }
    // The rail nodes and the pads age in real time.
    for node in &mut cx.pawn.magic.rails {
        node.life -= dt;
    }
    cx.pawn.magic.rails.retain(|node| node.life > 0.0);
    for pad in &mut cx.pawn.magic.pads {
        pad.life -= dt;
    }
    cx.pawn.magic.pads.retain(|pad| pad.life > 0.0);
}

fn step(cx: &mut Ctx, p: &mut Projectile, dt: f32, spawned: &mut Vec<SpawnArgs>, inherited: &mut Vec<Vec<TargetId>>) {
    // 1. The level impact of the previous tick (slot 528): map hit effect, then Explode(hit).
    if let Some((point, normal)) = p.pending_impact.take() {
        if !p.def.map_hit_effect.is_empty() {
            cx.spawn_effect(EffectSpawn { id: EffectId(NO_ID), name: p.def.map_hit_effect, at: Attach::World(point), rotation: Vec3::ZERO, scale: 1.0, persistent: false });
        }
        // (audio implementer) The class's map-hit SE (m_HitEffects[0].m_MapHitSEAsset), by class.
        cx.play_sound(SoundPlay { id: crate::host::audio::ACTOR_MAP_HIT, name: p.def.class_name, at: Attach::World(point), volume: 1.0, pitch: 1.0, looping: false, spatial: true });
        explode(cx, p, Some(HitWhat::Level(point, normal)), spawned, inherited);
        return;
    }
    // The barrier rides on Sora's root (I 3.1: KeepRelative).
    if let Some(offset) = p.attached {
        p.position = cx.pawn.cmc.location + rotate_yaw(offset, cx.pawn.yaw);
        // Pitched 60 degrees up in his frame (the spawn direction).
        let yaw = cx.pawn.yaw.to_radians();
        let pitch = Sheet::Spells.num("cast.shot.water_pitch").to_radians();
        p.forward = Vec3::new(yaw.cos() * pitch.cos(), yaw.sin() * pitch.cos(), pitch.sin());
    }
    // 2. Life counters.
    p.age += dt;
    p.distance += (p.position - p.last_position).length();
    p.last_position = p.position;
    // 3. Put effects (slot 538): every `m_fPutEffectDuration` cm from the fire position.
    if p.def.put_effect && p.def.put_duration > 0.0 {
        let travelled = if p.def.put_xy { (p.position - p.put_origin).truncate().length() } else { (p.position - p.put_origin).length() };
        let mut guard = 0;
        while travelled >= p.next_put && guard < 64 {
            let back = travelled - p.next_put;
            let at = p.position - p.forward * back;
            put_effect(cx, p, at, false);
            p.next_put += p.def.put_duration;
            guard += 1;
        }
    }
    // 4. Life over.
    let stuck = p.def.stuck_time > 0.0 && p.stuck_timer >= p.def.stuck_time;
    if (p.def.life > 0.0 && p.age > p.def.life) || (p.def.max_distance > 0.0 && p.distance > p.def.max_distance) || stuck {
        if p.def.explode_on_time_out {
            let (point, normal) = (p.position, -p.forward);
            explode(cx, p, Some(HitWhat::Level(point, normal)), spawned, inherited);
        } else {
            // Shutdown, attack collision off, Respawn(class, no hit), SpawnResidues(none).
            shutdown(cx, p);
            respawn(cx, p, None, spawned, inherited);
            residues(cx, p);
            p.dead = true;
        }
        return;
    }
    // 5. The class's own tick.
    match p.def.class {
        ProjClass::Cure => {
            // Tick 0x1414840A0: on the first tick after firing with a target: DoCure, shutdown.
            if p.hits == 0 {
                do_cure(cx, p);
                p.hits = 1;
                shutdown(cx, p);
                p.dead = true;
                return;
            }
        }
        ProjClass::Ragnarok => helix(p, dt),
        ProjClass::WaterBolt => water_bolt(cx, p),
        _ => {}
    }
    // 6. The movement component.
    movement(cx, p, dt);
    if p.dead {
        return;
    }
    // 7. The attack shapes.
    shapes(cx, p, spawned, inherited);
    // (HM-MAGIC) TimerTick (0x14062CFC0; magic spec I 0.2, as combat/hit.rs's windows) after the sweep: with an interval the
    // clock starts at the first hit and empties the whole hit list when it elapses, so every enemy still inside is hit again.
    if p.def.hit_interval > 0.0 && !p.dead {
        if p.hit_list.is_empty() {
            p.hit_timer = 0.0;
        } else {
            p.hit_timer += dt;
            if p.hit_timer >= p.def.hit_interval {
                p.hit_list.clear();
                p.hit_timer = 0.0;
            }
        }
    }
}

/// `OnPutEffect(P)`: Fire's scorch (cosmetic) and Blizzard's rail node (I 4.1).
fn put_effect(cx: &mut Ctx, p: &mut Projectile, at: Vec3, last: bool) {
    match p.def.class {
        ProjClass::Fire => {
            // Line trace 200 down; on ground: `vm_fir_zan1` + the decal (B 3.1). Cosmetic.
            if let Some(hit) = cx.world.line_trace(at, at - Vec3::Z * 200.0) {
                cx.spawn_effect(EffectSpawn { id: EffectId(NO_ID), name: "vm_fir_zan1", at: Attach::World(hit.position), rotation: Vec3::ZERO, scale: 1.0, persistent: false });
            }
        }
        ProjClass::Blizzard => {
            if p.def.rail_life <= 0.0 || p.rail_closed {
                p.rail_previous = None;
                p.rail_last_put = at;
                return;
            }
            // Not closed: a sphere of radius 25 swept from the last put to P; a blocking hit on
            // level geometry moves P to the hit location and closes the rail after this put.
            let mut at = at;
            let mut close_after = false;
            if (at - p.rail_last_put).length_squared() > 1.0e-4 {
                if let Some(hit) = cx.world.sweep_capsule(p.rail_last_put, at, 25.0, 25.0).filter(|h| !h.start_penetrating) {
                    at = p.rail_last_put + (at - p.rail_last_put) * hit.time;
                    close_after = true;
                }
            }
            p.rail_last_put = at;
            let last = last || close_after;
            // floor = first hit of the trace P + 100 -> P - m_GenerateRailHeight.
            let up = Sheet::Proj.num_or("blizzard.rail_trace_up", 100.0);
            let Some(floor) = cx.world.line_trace(at + Vec3::Z * up, at - Vec3::Z * p.def.rail_height).filter(|h| !h.start_penetrating) else {
                return;
            };
            let floor = floor.position;
            let mut allowed = true;
            if let Some((_, previous)) = p.rail_previous {
                if (previous - floor).length_squared() <= 100.0 {
                    allowed = false;
                } else if cx.world.sweep_capsule(previous + Vec3::Z * 70.0, floor + Vec3::Z * 70.0, 20.0, 40.0).is_some() {
                    // A wall between the two points: no node, the chain is broken.
                    allowed = false;
                    p.rail_previous = None;
                }
            } else if cx.world.sweep_capsule(floor + Vec3::Z * 100.0, floor, 60.0, 60.0).is_some_and(|h| h.start_penetrating) {
                // The first node: a sphere of radius 60 from 100 above the floor point starts
                // inside geometry: no node.
                allowed = false;
            }
            if allowed {
                let id = cx.pawn.magic.next_id();
                let (previous, tangent) = match p.rail_previous {
                    Some((previous_id, position)) => (previous_id, (floor - position).normalize_or_zero()),
                    None => (0, p.forward),
                };
                cx.pawn.magic.rails.push(RailNode { id, previous, position: floor, tangent, life: p.def.rail_life });
                cx.spawn_effect(EffectSpawn { id: EffectId(NO_ID), name: "TresRailSlideActor", at: Attach::World(floor), rotation: Vec3::ZERO, scale: 1.0, persistent: false });
                p.rail_previous = Some((id, floor));
            }
            if last {
                p.rail_closed = true;
            }
        }
        _ => {}
    }
}

/// Shutdown (slot 532): the particle is deactivated; Blizzard lays its last rail node.
fn shutdown(cx: &mut Ctx, p: &mut Projectile) {
    // (SHOT-A cannon) the owner's OnProjectileShutdown (notify interface +0x98) reads the actor as it is now
    cx.pawn.magic.shut_down.push((p.id, p.position, p.forward));
    if p.def.class == ProjClass::Blizzard && p.def.put_effect {
        let at = p.position;
        put_effect(cx, p, at, true);
    }
}

/// `Explode(hit)` (slot 573 0x141479100): shutdown, respawn, residues; the actor is gone (ExplodeLife 0).
/// (SHOT-FIX) Then slot 537 (0x141473E30): with `m_bAttachToHitActor` the actor is attached to the actor it hit
/// (KeepWorldTransform; `m_AttachToHitActorOffset` is set by no blueprint of Blueprints/Projectile: 0) and rides on it until its `m_ExplodeLife` has run out (the tick
/// after for 0: the +0x758 countdown); its body particle is deactivated by the Shutdown (vt+0xAE8). A level hit attaches
/// to static geometry: it stays where it is (the actor is gone as before).
fn explode(cx: &mut Ctx, p: &mut Projectile, hit: Option<HitWhat>, spawned: &mut Vec<SpawnArgs>, inherited: &mut Vec<Vec<TargetId>>) {
    shutdown(cx, p);
    respawn(cx, p, hit, spawned, inherited);
    residues(cx, p);
    p.dead = true;
    if let (true, Some(HitWhat::Pawn(id))) = (p.def.attach_to_hit_actor, hit) {
        if let Some(t) = cx.target(id) {
            p.attach_to_hit(&*cx.host.targets, &t); // (AUDIT-B W-FIN) the hit's bone
            if let Some(handle) = p.effect.take() {
                cx.stop_effect(handle, false);
            }
            p.linger = Some(p.def.explode_life.max(0.0));
            p.dead = false;
        }
    }
}

/// `Respawn(class, hit)` (slot 535; I 6.1).
fn respawn(cx: &mut Ctx, p: &mut Projectile, hit: Option<HitWhat>, spawned: &mut Vec<SpawnArgs>, inherited: &mut Vec<Vec<TargetId>>) {
    if !p.def.respawn_enabled {
        return;
    }
    let ok = match p.def.respawn_check {
        RespawnCheck::HitAlways => true,
        RespawnCheck::HitAny => hit.is_some(),
        RespawnCheck::HitPawn => matches!(hit, Some(HitWhat::Pawn(_))),
        RespawnCheck::HitMap => matches!(hit, Some(HitWhat::Level(..))),
        RespawnCheck::NoHit => hit.is_none(),
    };
    if !ok {
        return;
    }
    let Some(def) = p.def.successor() else { return };
    let direction = match p.def.respawn_rot {
        RotInherit::Normal => p.forward,
        RotInherit::YawOnly => Vec3::new(p.forward.x, p.forward.y, 0.0).normalize_or_zero(),
        RotInherit::Clear => Vec3::ZERO,
    };
    let _ = cx;
    spawned.push(SpawnArgs {
        def,
        position: p.position,
        direction,
        cmd: p.cmd,
        shoot_flow: p.shoot_flow,
        finish: p.def.take_finish && p.finish,
        situation: p.def.take_finish && p.situation,
        full_mp: p.full_mp,
        combo: p.combo,
        target: if p.def.take_target && p.has_target { p.target } else { TargetId::NONE },
        attached: None,
        fire: true,
    });
    inherited.push(if p.def.take_hit_list { p.hit_list.clone() } else { Vec::new() });
}

/// `SpawnResidues` (once per projectile): the Aero jump pad (I 2.3).
fn residues(cx: &mut Ctx, p: &mut Projectile) {
    if !p.def.residue || p.residue_spawned {
        return;
    }
    p.residue_spawned = true;
    let id = cx.pawn.magic.next_id();
    let life = Sheet::Proj.num("aero_cylinder.main_life");
    cx.pawn.magic.pads.push(AeroPad { id, position: p.position, life });
    cx.spawn_effect(EffectSpawn { id: EffectId(NO_ID), name: "vm_air_zan0", at: Attach::World(p.position), rotation: Vec3::ZERO, scale: 1.0, persistent: false });
}

/// `DoCure` 0x1414742A0 (B 3.6, I 5): the target gets `m_AppendCure + max(1, MagicPower)`; every
/// other pawn of the target's team within `m_DetectRange` of it gets `m_BonusAppendCure + the
/// same` (a dead one half, rounded up). The target of the port's casts is Sora.
fn do_cure(cx: &mut Ctx, p: &Projectile) {
    let Some((range, append, bonus)) = p.def.cure else { return };
    let stat = cx.pawn.status.magic().max(1);
    // Effect kinds 7 / 8 / 9 by tier for the target, 10 / 11 / 12 for the others (C 5).
    let tier = match p.def.family {
        "cure" => 0,
        "cura" => 1,
        _ => 2,
    };
    cx.pawn.status.recover(append + stat, 0, 0, 7 + tier);
    let centre = cx.pawn.cmc.location;
    // (audio implementer) the heal effect of the tier (it carries the heal's sound); was the
    // placeholder "sora_kaihuku".
    cx.spawn_effect(EffectSpawn { id: EffectId(NO_ID), name: crate::host::audio::cure_heal_effect(tier as usize), at: Attach::SORA, rotation: Vec3::ZERO, scale: 1.0, persistent: false });
    let mut allies = Vec::new();
    cx.host.targets.allies(&mut allies);
    for ally in allies {
        if (ally.position - centre).length() <= range {
            let mut amount = bonus + stat;
            if ally.dead {
                amount = (amount + 1) / 2;
            }
            cx.host.targets.heal_ally(ally.id, amount, 10 + tier);
        }
    }
}

/// The Ragnarok helix (G 3.3; actor Tick 0x1414845E0, before the base tick).
fn helix(p: &mut Projectile, dt: f32) {
    let rolling = p.def.rolling; // (SHOT-A) the class's values (were kc2.proj_kc_ragnarok.rolling_*)
    let apply = |p: &mut Projectile, dt: f32| {
        // The offset (0, 0, radius) rotated by the roll about the actor's forward axis.
        let forward = p.forward;
        let right = Vec3::new(-forward.y, forward.x, 0.0).normalize_or_zero();
        let right = if right == Vec3::ZERO { Vec3::Y } else { right };
        let up = right.cross(forward) * -1.0;
        let offset = |angle: f32, radius: f32| {
            let a = angle.to_radians();
            (up * a.cos() + right * a.sin()) * radius
        };
        let old = offset(p.helix_angle, p.helix_radius);
        p.helix_angle += dt * p.helix_speed;
        let new = offset(p.helix_angle, p.helix_radius);
        p.position = p.position - old + new;
    };
    let roll_speed = |p: &mut Projectile, dt: f32| {
        let max = rolling.speed_max;
        if p.helix_speed_add != 0.0 {
            p.helix_speed += p.helix_speed_add * dt;
            if (p.helix_speed_add < 0.0 && p.helix_speed <= max) || (p.helix_speed_add > 0.0 && p.helix_speed >= max) {
                p.helix_speed = max;
                p.helix_speed_add = 0.0;
            }
        }
    };
    match p.helix_phase {
        0 => {
            if p.active {
                p.helix_phase = 1;
                p.helix_radius = 0.0;
            }
        }
        1 => {
            let full = rolling.radius;
            p.helix_radius += dt * rolling.open_rate;
            if p.helix_radius > full {
                p.helix_radius = full;
                p.helix_phase = 2;
            }
            roll_speed(p, dt);
            apply(p, dt);
        }
        _ => {
            let max = rolling.radius_max;
            if p.helix_radius_add != 0.0 {
                p.helix_radius += p.helix_radius_add * dt;
                if (p.helix_radius_add < 0.0 && p.helix_radius <= max) || (p.helix_radius_add > 0.0 && p.helix_radius >= max) {
                    p.helix_radius = max;
                    p.helix_radius_add = 0.0;
                }
            }
            roll_speed(p, dt);
            apply(p, dt);
        }
    }
}

/// The Water bolt's native tick 0x1414841E0 (B 3.4): with a target, the first time the XY
/// distance is under 800 the turn rate is set once by distance; a target that stops being
/// targetable is dropped.
fn water_bolt(cx: &mut Ctx, p: &mut Projectile) {
    if !p.has_target {
        return;
    }
    match cx.target(p.target).filter(|t| t.flags.has(TargetFlags::ALIVE)) {
        None => p.has_target = false,
        Some(t) => {
            let d = (t.center - p.position).truncate().length();
            if !p.turn_set && d < 800.0 {
                p.turn_set = true;
                p.turn_max = if d < 350.0 {
                    1440.0
                } else if d < 550.0 {
                    1080.0
                } else {
                    720.0
                };
            }
        }
    }
}

/// The steering of 0x141481160 (B 2.2 step 3): turns `v` toward `to` by at most `max_turn`
/// radians; the speed is kept.
pub fn steer(v: Vec3, to: Vec3, max_turn: f32, cos_max_angle: f32) -> Vec3 {
    let speed = v.length();
    if speed <= 0.0 || to.length_squared() <= 1.0e-8 || max_turn <= 0.0 {
        return v;
    }
    let (vn, tn) = (v / speed, to.normalize());
    let c = vn.dot(tn).clamp(-1.0, 1.0);
    if c <= cos_max_angle {
        return v;
    }
    if c >= max_turn.cos() {
        return tn * speed;
    }
    // Rotated toward `tn` by 2 atan(sin(theta) tan(delta / 2)), theta = the angle between them,
    // about the (unnormalised, as in the game) axis vn x tn.
    let theta = c.acos();
    let angle = 2.0 * (theta.sin() * (max_turn * 0.5).tan()).atan();
    let axis = vn.cross(tn);
    let axis = if axis.length_squared() > 1.0e-12 {
        axis.normalize()
    } else {
        // Opposite directions: any axis across the velocity.
        let side = vn.cross(Vec3::Z);
        if side.length_squared() > 1.0e-12 { side.normalize() } else { Vec3::X }
    };
    let rotated = glam::Quat::from_axis_angle(axis, angle) * vn;
    rotated.normalize_or_zero() * speed
}

fn movement(cx: &mut Ctx, p: &mut Projectile, dt: f32) {
    // Move-start delay (0x141487FE0): the component does nothing while inactive.
    if p.move_delay > 0.0 {
        p.move_delay -= dt;
        if p.move_delay <= 0.0 {
            p.move_delay = 0.0;
            p.active = true;
        } else {
            p.active = false;
        }
    }
    if !p.active || p.attached.is_some() {
        return;
    }
    // A static actor: no homing, no move.
    if p.def.initial_speed <= 0.0 && p.velocity == Vec3::ZERO {
        return;
    }
    // Homing (0x1414751E0 -> 0x1414859E0 -> 0x141486CB0).
    if p.def.homing && p.homing_on {
        let mut point = None;
        if p.has_target {
            match cx.target(p.target).filter(|t| t.flags.has(TargetFlags::ALIVE)) {
                Some(t) => point = Some(t.center),
                // A dead / untargetable target: the flag is cleared, the bolt flies straight on.
                None => p.has_target = false,
            }
        }
        if p.homing_delay > 0.0 {
            p.homing_delay = (p.homing_delay - dt).max(0.0);
        }
        if p.homing_delay <= 0.0 {
            if point.is_none() && p.def.set_falling {
                // `m_bSetFallingIfNoHoming`: it homes to the point 1000 cm along its own down axis.
                let right = Vec3::new(-p.forward.y, p.forward.x, 0.0).normalize_or_zero();
                let up = if right == Vec3::ZERO { Vec3::X } else { right.cross(p.forward) * -1.0 };
                point = Some(p.position - up * 1000.0);
            }
            if let Some(point) = point {
                if p.turn_max > 0.0 {
                    p.turn_max += dt * p.def.turn_accel;
                    if p.def.max_turn_max > 0.0 {
                        p.turn_max = p.turn_max.min(p.def.max_turn_max);
                    } else if p.turn_max >= Sheet::Proj.num("proj_move.homing_turn_reset_limit") {
                        p.turn_max = 0.0;
                    }
                }
                let delta = point - p.position;
                if p.def.homing_min_dist > 0.0 && delta.length_squared() <= p.def.homing_min_dist * p.def.homing_min_dist {
                    // Inside the minimum distance: homing is switched off for good.
                    p.homing_on = false;
                } else {
                    let turn = if p.turn_max > 0.0 { (p.turn_max.to_radians() * dt).min(std::f32::consts::PI) } else { std::f32::consts::PI };
                    let cos_max = if p.def.homing_max_angle <= 0.0 { -1.0 } else { p.def.homing_max_angle.to_radians().cos() };
                    p.velocity = steer(p.velocity, delta, turn, cos_max);
                }
            }
        }
    }
    // ComputeVelocity (0x1414729E0): `v += normalize(v) * m_Accel * dt`, clamp to MaxSpeed;
    // the move is `v dt + (v' - v) dt / 2`.
    let v0 = p.velocity;
    let mut v1 = v0;
    if p.def.accel > 0.0 {
        let along = if v0.length_squared() > 0.0 { v0.normalize() } else { p.forward };
        v1 += along * p.def.accel * dt;
    }
    if p.def.max_speed > 0.0 && v1.length() > p.def.max_speed {
        v1 = v1.normalize() * p.def.max_speed;
    }
    let delta = v0 * dt + (v1 - v0) * dt * 0.5;
    p.velocity = v1;
    if v1.length_squared() > 1.0e-8 {
        p.forward = v1.normalize();
    }
    let start = p.position;
    let mut moved = delta;
    p.grazing = false;
    if p.def.root_radius > 0.0 && delta.length_squared() > 0.0 {
        if let Some(hit) = cx.world.sweep_capsule(start, start + delta, p.def.root_radius, p.def.root_radius) {
            let contact = start + delta * hit.time;
            let direction = delta.normalize();
            // HandleImpact 0x14147AD80: with the normal check, a hit shallower than the angle only
            // deflects (speed kept); otherwise, and for a start-penetrating hit, it stops.
            let head_on = -(direction.dot(hit.impact_normal));
            if !p.def.normal_check || hit.start_penetrating || head_on >= p.def.normal_angle.to_radians().cos() {
                p.position = contact;
                p.pending_impact = Some((hit.position, hit.impact_normal));
                p.velocity = Vec3::ZERO;
                p.active = false;
                return;
            }
            p.grazing = true;
            let speed = v1.length();
            let slid = v1 - hit.impact_normal * v1.dot(hit.impact_normal);
            if slid.length_squared() > 1.0e-8 {
                p.velocity = slid.normalize() * speed;
                p.forward = p.velocity.normalize();
            }
            let rest = p.velocity * dt * (1.0 - hit.time);
            // Lifted off the surface by a hair so that the next sweep does not start in it.
            let from = contact + hit.impact_normal * 0.1;
            let to = match cx.world.sweep_capsule(from, from + rest, p.def.root_radius, p.def.root_radius) {
                Some(second) => from + rest * second.time,
                None => from + rest,
            };
            moved = to - start;
        }
    }
    p.position = start + moved;
    // The stuck timer: grazing and hardly moving.
    let speed = p.velocity.length();
    if p.grazing && speed > 0.0 && moved.length() <= speed * Sheet::Proj.num("proj_move.stuck_move_ratio") * dt {
        p.stuck_timer += dt;
    } else {
        p.stuck_timer = 0.0;
    }
}

/// Closest points of two segments (as in combat/hit.rs).
fn closest(p1: Vec3, q1: Vec3, p2: Vec3, q2: Vec3) -> (Vec3, Vec3) {
    let (d1, d2, r) = (q1 - p1, q2 - p2, p1 - p2);
    let (a, e, f) = (d1.length_squared(), d2.length_squared(), d2.dot(r));
    if a <= 1.0e-8 && e <= 1.0e-8 {
        return (p1, p2);
    }
    let (s, t);
    if a <= 1.0e-8 {
        s = 0.0;
        t = (f / e).clamp(0.0, 1.0);
    } else {
        let c = d1.dot(r);
        if e <= 1.0e-8 {
            t = 0.0;
            s = (-c / a).clamp(0.0, 1.0);
        } else {
            let b = d1.dot(d2);
            let denominator = a * e - b * b;
            let mut s0 = if denominator > 1.0e-8 { ((b * f - c * e) / denominator).clamp(0.0, 1.0) } else { 0.0 };
            let mut t0 = (b * s0 + f) / e;
            if t0 < 0.0 {
                t0 = 0.0;
                s0 = (-c / a).clamp(0.0, 1.0);
            } else if t0 > 1.0 {
                t0 = 1.0;
                s0 = ((b - c) / a).clamp(0.0, 1.0);
            }
            s = s0;
            t = t0;
        }
    }
    (p1 + d1 * s, p2 + d2 * t)
}

const SWEEP_SAMPLES: usize = 4;

/// The attack-collision update: each shape swept from its previous centre to the new one
/// against the hurt shapes of every target not yet in the hit list (B 2.1, I 0.2).
fn shapes(cx: &mut Ctx, p: &mut Projectile, spawned: &mut Vec<SpawnArgs>, inherited: &mut Vec<Vec<TargetId>>) {
    let mut targets: Vec<TargetInfo> = Vec::new();
    let mut hurt: Vec<HurtShape> = Vec::new();
    for k in 0..2 {
        let Some(shape) = p.def.shapes[k] else { continue };
        let (radius, half) = shape.extent(p.age);
        let centre = p.position + shape.offset;
        let from = p.shape_previous[k].unwrap_or(centre);
        p.shape_previous[k] = Some(centre);
        if radius <= 0.0 {
            continue;
        }
        if targets.is_empty() {
            cx.targets(&mut targets);
        }
        // (time, target, part, point, normal), earliest contact per target.
        let mut hits: Vec<(f32, TargetInfo, u32, Vec3, Vec3)> = Vec::new();
        let samples = if from == centre { 0 } else { SWEEP_SAMPLES };
        for target in &targets {
            if !target.flags.has(TargetFlags::ALIVE | TargetFlags::HITTABLE) || p.hit_list.contains(&target.id) {
                continue;
            }
            hurt.clear();
            cx.hurt_shapes(target.id, &mut hurt);
            'target: for step in 0..=samples {
                let t = if samples == 0 { 0.0 } else { step as f32 / samples as f32 };
                let c = from.lerp(centre, t);
                let (a, b) = (c - Vec3::Z * half, c + Vec3::Z * half);
                for h in &hurt {
                    let (on_shape, on_hurt) = closest(a, b, h.a, h.b);
                    let gap = on_shape - on_hurt;
                    if gap.length() <= radius + h.radius {
                        let normal = if gap.length_squared() > 1.0e-8 { gap.normalize() } else { Vec3::Z };
                        hits.push((t, *target, h.part, on_hurt + normal * h.radius, normal));
                        break 'target;
                    }
                }
            }
        }
        hits.sort_by(|x, y| x.0.total_cmp(&y.0));
        for (time, target, part, point, normal) in hits {
            apply_hit(cx, p, &shape, &target, TargetHit { part, point, normal, time });
            if p.def.no_destroy_on_hit {
                // A static actor hits every enemy inside once; its residue is spawned at the
                // first reported hit.
                residues(cx, p);
            } else {
                // A moving bolt dies on its first character hit: Explode(hit).
                explode(cx, p, Some(HitWhat::Pawn(target.id)), spawned, inherited);
                return;
            }
        }
    }
}

fn hit_effect_of(attribute: ETresDamageAttribute) -> &'static str {
    match attribute {
        ETresDamageAttribute::Fire => "vs_fir_hit0",
        ETresDamageAttribute::Blizzard => "vs_blz_hit0",
        ETresDamageAttribute::Thunder => "vs_thd_hit0",
        ETresDamageAttribute::Water => "vs_wat_hit0",
        ETresDamageAttribute::Aero => "vs_win_hit0",
        ETresDamageAttribute::Dark => "vs_dak_hit1",
        _ => "VS_non_HIT0",
    }
}

/// The damage of one hit of `row` on `target` (C 4.2 through `status::compute_damage`).
pub(crate) fn damage_of(cx: &Ctx, target: TargetId, row: Option<&AttackRow>, full_mp: bool, combo_step: i32) -> f32 {
    let status = &cx.pawn.status;
    let attack = DamageAttack {
        magic: row.is_none_or(|r| r.magic),
        second_resist: row.is_some_and(|r| r.rapid_fire),
        full_mp,
        combo_step: combo_step.clamp(0, 99) as i8,
        recover: row.is_some_and(|r| r.kind == ETresDamageKind::Recover),
        ..DamageAttack::magic(row.map_or(1.0, |r| r.power), row.map_or(ETresDamageAttribute::Notype, |r| r.attribute))
    };
    let defender = Defender::of_target(cx.host.targets.defense(target).as_ref());
    // (XSI) Hidden Potential's floor against a powerful foe (abilities/xs).
    let mut attacker = status.as_attacker();
    (attacker.strength, attacker.magic) = crate::abilities::xs::overcome_stats(cx, target, attacker.strength, attacker.magic);
    compute_damage(&attacker, &defender, &attack, &DamageContext::of(status)).amount
}

/// Where a shape met a target, and when in the step.
#[derive(Clone, Copy)]
struct TargetHit {
    part: u32,
    point: Vec3,
    normal: Vec3,
    time: f32,
}

/// One hit of a shape on a target: the hit record, the damage, effects, the bookkeeping.
fn apply_hit(cx: &mut Ctx, p: &mut Projectile, shape: &ShapeDef, target: &TargetInfo, hit: TargetHit) {
    let TargetHit { part, point, normal, time } = hit;
    let name = p.row_name(shape);
    let row = attack_row(name);
    let counters = p.combo.map(|c| c.clamp(0, 99) as u8);
    let mut info = HitInfo { counters, victim_result: ReactionResult::Damage, attacker_result: ReactionResult::NoReaction, impact_normal: normal, time, ..HitInfo::DEFAULT };
    let mut kind = ETresDamageKind::Small;
    let mut attribute = ETresDamageAttribute::Notype;
    if let Some(r) = &row {
        kind = r.kind;
        attribute = r.attribute;
        info = HitInfo { power: r.power, power_scale: r.power_scale, parabola_angle: r.parabola_angle, effect_time: r.effect_time, reaction_power: r.reaction_power, armor_attack_power: r.armor_attack, form_point: r.form_point, revenge_point: r.revenge_point, phys_force: r.phys_force, ..info };
        info.flags50 |= (r.magic as u8) | (r.killer as u8) << 2 | (r.invalid_guard as u8) << 3 | (r.rapid_fire as u8) << 4;
        info.flags51 |= r.attraction as u8;
        // The status roll (`m_BadStatusGenRate` percent; C 6): the port's own random stream.
        if r.bad_status != "NONE" && r.bad_rate > 0.0 {
            let roll = (cx.pawn.magic.next_random() % 100) as f32;
            if roll < r.bad_rate {
                info.bad_status = r.bad_status;
                info.bad_time = r.bad_time;
            }
        }
    }
    if p.finish || p.situation {
        info.flags50 |= HitInfo::FLAG50_FINISH;
    }
    if p.full_mp {
        info.flags51 |= 0x10;
    }
    // MAGIC_DRAW (60, Magic Treasure Magnet; hit dispatch 0x140618940, C 4.1): the attacker has
    // the ability and the attack is magic: `m_bIsEnableMagicDraw` (info+0x51 bit 5): the prizes
    // this hit knocks out are drawn to him (the prize side reads the flag; no damage effect).
    if row.as_ref().is_none_or(|r| r.magic) && cx.pawn.status.has(ETresAbilityKind::MagicDraw) {
        info.flags51 |= 0x20;
    }
    // Aero: once the swirl limit was reached the later victims get `m_AfterDamageKind` (I 2.1).
    if let Some(after) = p.after_kind {
        kind = after;
    }
    // info+0x54 = the second value of the combo setter (state+0xBC: counter A at Begin).
    let amount = damage_of(cx, target.id, row.as_ref(), p.full_mp, p.combo[2]);
    // The knockback direction: along the bolt's flight; for a static actor from its centre.
    let direction = if p.velocity.length_squared() > 1.0 {
        p.velocity.normalize()
    } else {
        let d = target.position - p.position;
        Vec3::new(d.x, d.y, 0.0).normalize_or_zero()
    };
    p.hit_list.push(target.id);
    p.hits += 1;
    let effect = if p.def.hit_effect.is_empty() { hit_effect_of(attribute) } else { p.def.hit_effect };
    cx.spawn_effect(EffectSpawn { id: EffectId(NO_ID), name: effect, at: Attach::World(point), rotation: Vec3::ZERO, scale: 1.0, persistent: false });
    if !p.def.hit_sound.is_empty() {
        cx.play_sound(SoundPlay { id: SoundId(NO_ID), name: p.def.hit_sound, at: Attach::World(point), volume: 1.0, pitch: 1.0, looping: false, spatial: true });
    } else {
        // (audio implementer) The class's own pawn-hit SE (m_HitEffects[0].m_PawnHitSEAsset):
        // named by the actor class, the audio side has the table (host/audio.rs).
        cx.play_sound(SoundPlay { id: crate::host::audio::ACTOR_PAWN_HIT, name: p.def.class_name, at: Attach::World(point), volume: 1.0, pitch: 1.0, looping: false, spatial: true });
    }
    let record = HitRecord { target: target.id, part, attack: u32::MAX, attack_name: name, position: point, direction, damage: amount, damage_kind: kind, attribute, impulse: Vec3::ZERO, attacker: crate::host::HitAttacker { position: cx.pawn.cmc.location, yaw: cx.pawn.yaw, radius: crate::params::SORA_PARAMS.capsule_radius, valid: true }, flags: p.cmd as u32 | (p.shoot_flow as u32) << 8, info, attack_info: Default::default() /* (GOOFY) */ };
    let result = if amount != 0.0 { cx.deliver_hit(&record) } else { Default::default() };
    // The Aero counter (notify 0x14147D060), after the damage call.
    if p.def.class == ProjClass::Aero {
        p.swirl_count += 1;
        if p.swirl_limit > 0 && p.swirl_count >= p.swirl_limit {
            p.after_kind = Some(ETresDamageKind::Blow);
            p.swirl_limit = -1;
        }
    }
    // A hit that deals damage feeds the formchange gauge and the element tallies (I 7.1).
    if result.damage_dealt > 0.0 {
        super::grand::on_damage(cx, info.form_point, p.cmd);
    }
}
