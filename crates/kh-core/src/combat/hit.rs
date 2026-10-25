//! Hit windows, hit detection and hit application: `UTresAtkCollComponent` as the second research
//! pass read it (`build/sora_tmp/combat/P2_C_section.md`, which replaces sections 8.1 / 8.2 of
//! docs/sora-combat-spec.md and adds 8.5 - 8.7; "C 8.x" below).
//!
//! What is here:
//! - activation / deactivation of a shape group by `TresAnimNotifyState_Attack` (C 8.2.1), with
//!   the Attack state's pre-processor (row rename, counters, finish / aerial flags);
//! - per tick (C 8.2.2, 8.2.3): the shape from the keyblade's sockets (generated per-frame socket
//!   tracks of the clip, actor space), size over time, the first tick in place, later ticks swept
//!   from the previous position; already-hit targets are excluded; `AttackInterval` is ONE timer
//!   per window that starts at the first hit and empties the whole hit list;
//! - per hit (C 8.2.5 steps 16 - 21, 8.5, 8.6, 8.7): the hit record, the damage (the status
//!   implementer's `compute_damage`), the knockback direction, effect and sound seam events, the
//!   on-contact and on-damage bookkeeping.
//!
//! Second pass (2026-10-11):
//! - the guard / armour / body classification (C 8.2.6 `Classify`, `classify.rs`) with the
//!   victim's answers from `Targets::body`; the results drive the effect choice (C 8.6), whether
//!   `TakeDamage` is called, and the on-contact handler (C 8.5.1): a repelled swing leaves the
//!   recoil record the Attack state's slot 129 turns into the Reflect state;
//! - map hits (C 8.2.5 step 9): only shapes whose `m_MapHitType` is HITMAP / IGNORE_GROUND
//!   sweep the world (`KB_Stick` of the base combo); the gate is the instruction-level part
//!   (floor-like surfaces with dot(Up, ImpactNormal) >= cos 50 are ignored, physical shapes
//!   ignore contacts not higher than 50 cm above his root); effect `VW_SO010_WHT0`, no sound.
//!   The world sweep is three spheres of the shape's radius along the blade (the host's
//!   `World` has upright shapes only). C-LEVEL REMAINDER: the Attack state's own map-hit
//!   handler (owner notify slot 24 0x141342760: the low-wall rule at 45 cm, ceilings, the
//!   reflect window) and the map-hit recoil (slot 140): the port takes slot 24's answer as 0;
//! - clashes (C 8.2.4): the pass against the host's attack shapes (`Targets::attack_shapes`)
//!   with the instruction-level result rule (the other shape's reflect level higher -> REFLECT,
//!   else GUARD_REFLECT; the row's `m_ReflectReaction`); a clashed window stops sweeping until
//!   its interval elapses (or for good without one). Third pass (2026-10-12): the sort's
//!   tie-break and the manager's real clash / guard effects (`EFFECT_*`). Not ported: the
//!   same-projectile test and the owner chain in the clash list (no projectiles of the host's
//!   attackers are known to the core).
//!
//! Still fixed:
//! - the tick order of the attack-collision manager against the pawn is not known (C open
//!   question 1): here the windows are ticked after the movement of the frame, before the state.
//! - The sweep is sampled (5 positions between the previous and the current shape), not an
//!   exact shape cast.

use glam::Vec3;

use super::classify::{classify, MapResponse, Shape};
use super::tables::{attack_row, group_shapes, AttackDataRow, P};
use crate::anim::{NotifyData, NotifyPhase, Notifies, SEQUENCES, SOCKET_VFX_CURE, SOCKET_VFX_CURE_PLUS_200};
use crate::enums::{ETresDamageAttribute, ETresDamageKind};
use crate::host::{Attach, AttackShape, EffectId, EffectSpawn, EventKind, HitInfo, HitRecord, HurtShape, ReactionResult, SoundId, SoundPlay, TargetFlags, TargetId, TargetInfo};
use crate::notify::{notify_asset_id, NOTIFY_ID_NONE};
use crate::pawn::Ctx;
use crate::status::{compute_damage, Attacker, DamageAttack, DamageContext, Defender, Side};

/// What the Attack state's pre-processor (0x141343800; C 8.2.1) puts into an activation. The
/// state keeps it current on the pawn while it runs; without an attack state the activation
/// gets zeros.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct AttackCtx {
    pub kind: u8,
    /// state+0x128, +0x12C, +0x130, +0x134.
    pub counters: [i32; 4],
    pub finish_end: bool,
    /// state+0x234 bit 2 (air mode).
    pub aerial: bool,
    /// state+0x23C bit 0 (a just-guard attack -> `info.justGuard`).
    pub just_guard: bool,
    /// The state has a homing target (the map-hit recoil needs "no target").
    pub has_target: bool,
    /// The state's target pawn (+0x110): Final Blow's chase needs the counter to hit IT.
    pub target: TargetId,
    /// An attack state filled this in (false: a window opened outside one).
    pub state: bool,
}

/// One activated shape group: the activation record and its primitive (C "rec" and "prim").
/// (D6b: not `Copy`: the hit list is the record's TArray, rec+0x18 / count +0x20, without a cap.)
#[derive(Clone, Debug, PartialEq)]
pub struct Window {
    /// The notify that opened it: (index into `SEQUENCES`, index of the notify).
    pub key: (u16, u16),
    pub group: &'static str,
    /// The row's index in `ATTACK_DATA` and name; `u32::MAX` / the notify's name when the table
    /// has no such row (the static default row applies: C 8.2.1).
    pub row: u32,
    pub row_name: &'static str,
    pub hit_se: Option<&'static str>,
    pub hit_se_asset: Option<&'static str>,
    /// (party stream D6) The actor's own `m_HitEffects` entry (group `None`, `m_PawnHitEffect`)
    /// for a shot: replaces the attribute's default of the chooser 0x140628960 (sora-vfx-spec 2.6).
    pub hit_effect: Option<&'static str>,
    /// (party stream D6b) `m_fHitEffectRandomOffsetRadius` of the shot's component (0: none).
    pub hit_effect_offset: f32,
    pub interval: f32,
    pub timer: f32,
    /// prim+0xA4C: time since activation.
    pub time: f32,
    initialised: bool,
    /// The shape's end points of the previous tick, world.
    previous: [Vec3; 2],
    /// Groups whose location is frozen after the first tick (`m_bDisableLocation1Attach`).
    frozen: bool,
    pub info: AttackCtx,
    /// (D6b) Every actor hit since the activation / the last interval: rec+0x18 is a TArray (no 16 cap).
    hit_list: Vec<TargetId>,
    /// Hits this activation has scored (for tests and the debug text).
    pub hits: u32,
    /// prim+0x986 bit 7, "no clash" (C 8.2.1): the row's reflect level <= 0, or any swing after
    /// the first of a combo.
    pub no_clash: bool,
    /// rec flag bit 1: the window clashed and stops sweeping (until its interval elapses).
    pub clashed: bool,
    /// The level geometry is "in the hit list": one map hit per activation (the null actor).
    map_hit: bool,
    clash_list: [TargetId; 4],
}

/// A shot an attack spawned (`TresAnimNotify_Shot` / the dive's landing -> the Attack state's
/// shot handler 0x141344580 -> pawn vt+0x1450(name, location, direction, team)): an actor of
/// Sora's projectile set with its own attack-collision component. Spec 8.8.
///
/// The only one of the Kingdom Key's base move set is `DOWNIMPACT` (blueprint
/// `/Game/Blueprints/Projectile/TresProj_DownImpact` [A]): it does not move (`InitialSpeed` 0),
/// lives `m_MainLife` 0.5 s, is not destroyed by a hit, and carries one SPHERE (group None, row
/// `KB_DOWN_BOMB`, radius 50 growing by 800 a second to 200, no map hits, not physical, no
/// interval: every target once); its particle `vp_hit_zmn0` sits 75 cm under its root.
#[derive(Clone, Debug, PartialEq)]
pub struct Shot {
    pub name: &'static str,
    /// The actor's location.
    pub position: Vec3,
    /// (party stream D6b) The actor's yaw (degrees): the shape's relative location and a CONVEX
    /// prism turn with it.
    pub yaw: f32,
    /// Seconds left of `m_MainLife`.
    pub life: f32,
    /// (`m_Size.X`, `m_IncSize.X`, `m_IncMaxSize.X`).
    size: (f32, f32, f32),
    /// (party stream D6b) `m_SizeVectorCurve` X keys (time, value, ...; linear): replaces `size`.
    curve: &'static [f32],
    /// The shot's primitive: row, hit list, the counters the spawning attack handed over
    /// (shot vt+0x1130 / +0x1140 / +0x1120 / +0x1180: combo, the three hit counters, finish end,
    /// just guard).
    pub window: Window,
    /// (party stream D6) The shape's `m_RelativeLocation1` in the actor's frame: the shape update
    /// 0x140647950 puts the shape at parent transform x relative location.
    pub offset: Vec3,
    /// (party stream D6) A CONVEX shape (`ETresCollision::CONVEX` 6) instead of the sphere.
    pub convex: Option<Convex>,
    /// (party stream D6b) The actor is attached to a friend (FUSION_SPIN: slot 138 0x14134C710
    /// attaches it to Goofy's mesh keeping its world transform, FUN_143E8FC60 with the rules
    /// DAT_148750ED0 = KeepWorldTransform, socket None): it follows his actor transform.
    pub attach: Option<ShotAttach>,
}

/// (party stream D6b) A shot carried by a friend: its location and yaw in the friend's frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShotAttach {
    /// `ETresChrUniqueID` of the friend (12 Goofy).
    pub uid: u8,
    pub local: Vec3,
    pub yaw: f32,
}

/// Turns `v` by `yaw` degrees about Z.
fn yawed(v: Vec3, yaw: f32) -> Vec3 {
    let (sin, cos) = yaw.to_radians().sin_cos();
    Vec3::new(v.x * cos - v.y * sin, v.x * sin + v.y * cos, v.z)
}

impl Shot {
    /// The sphere's radius now.
    pub fn radius(&self) -> f32 {
        if let Some(r) = curve_at(self.curve, self.window.time) {
            return r;
        }
        (self.size.0 + self.size.1 * self.window.time).min(self.size.2.max(self.size.0))
    }

    /// The shape's centre now.
    pub fn centre(&self) -> Vec3 {
        self.position + yawed(self.offset, self.yaw)
    }

    /// (party stream D6) The CONVEX shape now (`None`: a sphere shot).
    pub fn prism(&self) -> Option<Prism> {
        self.convex.map(|c| c.prism(self.centre(), self.window.time))
    }
}

/// (party stream D6b) A linear curve of (time, value) keys, clamped outside its keys
/// (FUN_140649E40 with `m_SizeVectorCurve`); `None` without keys.
fn curve_at(keys: &[f32], t: f32) -> Option<f32> {
    let n = keys.len() / 2;
    if n == 0 {
        return None;
    }
    let key = |i: usize| (keys[2 * i], keys[2 * i + 1]);
    if t <= key(0).0 {
        return Some(key(0).1);
    }
    for i in 1..n {
        let ((t0, v0), (t1, v1)) = (key(i - 1), key(i));
        if t <= t1 {
            return Some(if t1 > t0 { v0 + (v1 - v0) * (t - t0) / (t1 - t0) } else { v1 });
        }
    }
    Some(key(n - 1).1)
}

/// (party stream D6) A CONVEX attack shape of `Collision/tres_colltest01` (the StaticMesh's hull:
/// a 20-gon prism, 40 vertices at radius 100, angles k x 18 deg, z -50..+50 [A; kh-party
/// actions.rs `Prism` read it]) under the shape's growing scale. The shape update 0x140647950
/// case 6 / 8 (CONVEX / DCONVEX) keeps the scale (0x8A0..0x8A8 = FUN_140649BF0's growth x the
/// parent's scale, here the actor's 1); the SPHERE / CAPSULE cases 0 / 1 / 3 / 4 / 5 instead
/// multiply the size by the parent's smallest scale and reset the shape's scale to 1 (so a
/// sphere shot's `m_Scale` / `m_IncScale` change nothing) [C].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Convex {
    /// `m_Scale`, `m_IncScale`, `m_IncMaxScale`.
    pub scale: Vec3,
    pub inc_scale: Vec3,
    pub max_scale: Vec3,
    /// The actor's yaw (degrees): `m_RotAttachType` TRES_ACRA_NORMAL, `m_RelativeRocation` 0.
    pub yaw: f32,
}

impl Convex {
    /// FUN_140649BF0 [C]: with any `m_IncScale` component non-zero, each axis
    /// max(base + inc x t, 0.01), then capped by `m_IncMaxScale` where that is > 0; else `m_Scale`.
    pub fn scale_at(&self, t: f32) -> Vec3 {
        if self.inc_scale == Vec3::ZERO {
            return self.scale;
        }
        let axis = |base: f32, inc: f32, max: f32| {
            let v = (base + inc * t).max(0.01);
            if max > 0.0 { v.min(max) } else { v }
        };
        Vec3::new(axis(self.scale.x, self.inc_scale.x, self.max_scale.x), axis(self.scale.y, self.inc_scale.y, self.max_scale.y), axis(self.scale.z, self.inc_scale.z, self.max_scale.z))
    }

    pub fn prism(&self, centre: Vec3, t: f32) -> Prism {
        let s = self.scale_at(t);
        Prism { centre, radius: Vec3::new(100.0 * s.x, 100.0 * s.y, 0.0), half_height: 50.0 * s.z, yaw: self.yaw }
    }
}

/// (party stream D6) `tres_colltest01` placed in the world: an upright 20-gon prism.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Prism {
    pub centre: Vec3,
    /// The vertices' radius along the shape's local x / y (z unused), cm.
    pub radius: Vec3,
    pub half_height: f32,
    /// The angle of vertex 0, degrees.
    pub yaw: f32,
}

impl Prism {
    pub const SIDES: usize = 20;

    fn vertex(&self, i: usize) -> glam::Vec2 {
        let a = (i % Self::SIDES) as f32 * std::f32::consts::TAU / Self::SIDES as f32;
        let local = glam::Vec2::new(a.cos() * self.radius.x, a.sin() * self.radius.y);
        let (sin, cos) = self.yaw.to_radians().sin_cos();
        glam::Vec2::new(local.x * cos - local.y * sin, local.x * sin + local.y * cos)
    }

    /// The point of the prism nearest to `p` (`p` itself inside).
    pub fn closest_point(&self, p: Vec3) -> Vec3 {
        let d = p - self.centre;
        let z = d.z.clamp(-self.half_height, self.half_height);
        let q = glam::Vec2::new(d.x, d.y);
        let mut inside = true;
        let mut best = (f32::MAX, q);
        for i in 0..Self::SIDES {
            let (a, b) = (self.vertex(i), self.vertex(i + 1));
            let e = b - a;
            // The vertices run counter-clockwise: the outward normal of an edge is (e.y, -e.x).
            if (q - a).dot(glam::Vec2::new(e.y, -e.x)) > 0.0 {
                inside = false;
            }
            let t = ((q - a).dot(e) / e.length_squared()).clamp(0.0, 1.0);
            let c = a + e * t;
            let dist = q.distance_squared(c);
            if dist < best.0 {
                best = (dist, c);
            }
        }
        let xy = if inside { q } else { best.1 };
        self.centre + Vec3::new(xy.x, xy.y, z)
    }

    pub fn distance(&self, p: Vec3) -> f32 {
        p.distance(self.closest_point(p))
    }

    /// The point of segment `a`..`b` nearest to the prism (the distance along a line to a convex
    /// body is convex: golden-section search).
    pub fn nearest_on_segment(&self, a: Vec3, b: Vec3) -> Vec3 {
        if a.distance_squared(b) < 1.0e-6 {
            return a;
        }
        let f = |t: f32| self.distance(a.lerp(b, t));
        let (mut lo, mut hi) = (0.0f32, 1.0f32);
        let g = 0.618_034f32;
        let (mut x1, mut x2) = (hi - g * (hi - lo), lo + g * (hi - lo));
        let (mut f1, mut f2) = (f(x1), f(x2));
        for _ in 0..32 {
            if f1 <= f2 {
                hi = x2;
                (x2, f2) = (x1, f1);
                x1 = hi - g * (hi - lo);
                f1 = f(x1);
            } else {
                lo = x1;
                (x1, f1) = (x2, f2);
                x2 = lo + g * (hi - lo);
                f2 = f(x2);
            }
        }
        let t = [0.0, 1.0, (lo + hi) * 0.5].into_iter().min_by(|x, y| f(*x).total_cmp(&f(*y))).unwrap_or(0.0);
        a.lerp(b, t)
    }
}

/// Spawns shot `name` for the running attack (`info` = its pre-processor data). `at` is the shot
/// location the attack state asks the pawn for (vt+0x1410: the keyblade's ATK_01, or with an
/// offset): the base move set's shots stand there; the team finishers' actors are placed by their
/// spawn rule (sheet `sora-combat-shots`) and play their fire effects there.
/// Names without a known blueprint are announced as an effect only.
pub(crate) fn spawn_shot(cx: &mut Ctx, name: &'static str, at: Vec3, info: AttackCtx) {
    // The shot is not the Attack state's own primitive: no recoil record, no clash, no map hit.
    let info = AttackCtx { state: false, aerial: false, ..info };
    // (row, life, size, particle, particle offset) of the Kingdom Key's base move set.
    let base: Option<(&'static str, f32, (f32, f32, f32), &'static str, Vec3)> = match name {
        "DOWNIMPACT" => Some(("KB_DOWN_BOMB", 0.5, (50.0, 800.0, 200.0), "vp_hit_zmn0", Vec3::new(0.0, 0.0, -75.0))),
        // `TresProj_SonicJumpEx` [A]: `m_MainLife` 0.5, not destroyed by a hit, one SPHERE row
        // `FF_SONIC_BOMB`, radius 100 growing by 800 a second to 350; particle `vp_snc_hit0` at
        // its root. (Its movement - initial speed 100, homing after 0.1 s - is not ported: it
        // would travel about 50 cm in its life.)
        "SHOCKWAVE_S" => Some(("FF_SONIC_BOMB", 0.5, (100.0, 800.0, 350.0), "vp_snc_hit0", Vec3::ZERO)),
        _ => None,
    };
    if let Some((row, life, size, particle, offset)) = base {
        let (index, row_name) = attack_row(row).map_or((u32::MAX, row), |(i, r)| (i as u32, r.id));
        let window = Window { key: (u16::MAX, u16::MAX), group: name, row: index, row_name, hit_se: None, hit_se_asset: None, hit_effect: None, hit_effect_offset: 0.0, interval: 0.0, timer: 0.0, time: 0.0, initialised: true, previous: [at; 2], frozen: true, info, hit_list: Vec::new(), hits: 0, no_clash: true, clashed: false, map_hit: true, clash_list: [TargetId::NONE; 4] };
        cx.spawn_effect(EffectSpawn { id: EffectId(notify_asset_id(particle)), name: particle, at: Attach::World(at + offset), rotation: Vec3::ZERO, scale: 1.0, persistent: false });
        let yaw = cx.pawn.yaw;
        let combat = &mut cx.pawn.combat;
        combat.shots.push(Shot { name, position: at, yaw, life, size, curve: &[], window, offset: Vec3::ZERO, convex: None, attach: None });
        combat.shots_spawned += 1;
        return;
    }
    // (party stream D, re-read D6 / D6b) The team finishers' actors of `p_ex001_ProjectileSet`:
    // one row per attack shape in sheets/port/sora-combat-shots.json (tools/sora_combat_shots_sheet.js
    // from the blueprints and their parents [A]). Static actors (`InitialSpeed` 0,
    // `m_bNoDestroyOnHit`); every shape auto-activated at the spawn with its group's interval.
    let rows = || super::tables::SHOT_SHAPES.iter().filter(move |r| r.shot == name);
    let Some(first) = rows().next() else {
        cx.pawn.note("an attack asked for a shot whose blueprint this port does not know: it is announced as an effect only");
        cx.spawn_effect(EffectSpawn { id: EffectId(NOTIFY_ID_NONE), name, at: Attach::World(at), rotation: Vec3::ZERO, scale: 1.0, persistent: false });
        return;
    };
    let root = cx.pawn.cmc.location;
    // Placement [C]: KH3's root (+0x2B0) is Sora's feet (movement spec 5.1) = `cmc.location`.
    let (position, yaw, attach) = match first.spawn {
        // Slot 138 0x14134C710 (FUSION_SPIN): only while Goofy (pawn vt+0xF08(12)) lives (vt+0x17B0 =
        // his HitPoint); the actor at Sora's root with GOOFY's root rotation, then attached to
        // Goofy's mesh (vt+0xB58 = MyMesh +0x580) keeping its world transform.
        "ROOT_ATTACHED_GOOFY" => {
            let Some(goofy) = cx.pawn.combat.friend.friend(12).copied().filter(|f| !f.ko) else { return };
            let local = yawed(root - goofy.position, -goofy.yaw);
            (root, goofy.yaw, Some(ShotAttach { uid: 12, local, yaw: 0.0 }))
        }
        // Slot 137 0x14134CCE0 (UNISON_*): Sora's root + 100 up, his root rotation.
        _ => (root + Vec3::Z * 100.0, cx.pawn.yaw, None),
    };
    if !first.particle.is_empty() {
        let p = first.particle_loc;
        cx.spawn_effect(EffectSpawn { id: EffectId(notify_asset_id(first.particle)), name: first.particle, at: Attach::World(position + yawed(Vec3::new(p[0], p[1], p[2]), yaw)), rotation: Vec3::new(0.0, yaw, 0.0), scale: 1.0, persistent: false });
    }
    if !first.fire_effect.is_empty() {
        // SpawnFireEffects (vt+0x11F8 = slot 575 0x141482ED0, called by the slot with the shot
        // location): every `MyFireEffects` entry there with the actor's rotation, its sound there.
        cx.spawn_effect(EffectSpawn { id: EffectId(notify_asset_id(first.fire_effect)), name: first.fire_effect, at: Attach::World(at), rotation: Vec3::new(0.0, yaw, 0.0), scale: 1.0, persistent: false });
        if !first.fire_se.is_empty() {
            cx.play_sound(SoundPlay { id: SoundId(notify_asset_id(first.fire_se)), name: first.fire_se, at: Attach::World(at), volume: 1.0, pitch: 1.0, looping: false, spatial: true });
        }
    }
    let v3 = |v: &[f32]| Vec3::new(v[0], v[1], v[2]);
    for row in rows() {
        let group = if row.group == "None" { name } else { row.group };
        let (index, row_name) = attack_row(row.row).map_or((u32::MAX, row.row), |(i, r)| (i as u32, r.id));
        let opt = |s: &'static str| (!s.is_empty()).then_some(s);
        let offset = v3(row.loc);
        let window = Window { key: (u16::MAX, u16::MAX), group, row: index, row_name, hit_se: None, hit_se_asset: opt(row.hit_se), hit_effect: opt(row.hit_effect), hit_effect_offset: row.hit_offset, interval: row.interval, timer: 0.0, time: 0.0, initialised: true, previous: [position + yawed(offset, yaw); 2], frozen: true, info, hit_list: Vec::new(), hits: 0, no_clash: true, clashed: false, map_hit: true, clash_list: [TargetId::NONE; 4] };
        let convex = match (row.shape, row.mesh) {
            ("SPHERE", _) => None,
            ("CONVEX", "tres_colltest01") => Some(Convex { scale: v3(row.scale), inc_scale: v3(row.inc_scale), max_scale: v3(row.max_scale), yaw }),
            _ => {
                cx.pawn.note("a team shot's shape is neither a SPHERE nor tres_colltest01: not ported");
                continue;
            }
        };
        let combat = &mut cx.pawn.combat;
        combat.shots.push(Shot { name: group, position, yaw, life: row.life, size: (row.size[0], row.inc_size[0], row.max_size[0]), curve: row.size_curve, window, offset, convex, attach });
        combat.shots_spawned += 1;
    }
}

/// (GOOFY-GK, the Trinity Guard audit) The link pawn's own `m_HitEffects` for a team shot row: ShieldGuardian_Pawn
/// (Blueprints/FriendLink/ShieldGuardian_Pawn: group "Atk" m_PawnHitEffect VN_slg_hit0, group "Finish" vs_fir_hit0, no
/// m_PawnHitSEAsset: the effect's SeAttach plays the sound) [A]. Before, Trinity Guard's bashes showed the Kingdom
/// Key's physical hit vw_so010_hit0 (the attribute default of step 20 below). Other team rows keep that default.
fn team_shot_hit_effect(row: &str) -> Option<&'static str> {
    match row {
        "FR_SHIELDGUARDIAN_SHIELDBASH" | "FR_SHIELDGUARDIAN_SHIELDBASH2" => Some("VN_slg_hit0"),
        "FR_SHIELDGUARDIAN_HEAVYBASH" => Some("vs_fir_hit0"),
        _ => None,
    }
}

/// (party stream D) A team attack's hit sphere as one of Sora's shots (combat/friend.rs `FrLink`):
/// the row of `p_ex001_AttackData`, the info of no attack state (combo 0).
pub(crate) fn spawn_team_shot(cx: &mut Ctx, shot: &crate::host::TeamShot) {
    let (index, row_name) = attack_row(shot.row).map_or((u32::MAX, shot.row), |(i, r)| (i as u32, r.id));
    let info = AttackCtx { state: false, aerial: false, ..AttackCtx::default() };
    let window = Window { key: (u16::MAX, u16::MAX), group: shot.row, row: index, row_name, hit_se: None, hit_se_asset: None, hit_effect: team_shot_hit_effect(shot.row), hit_effect_offset: 0.0, interval: 0.0, timer: 0.0, time: 0.0, initialised: true, previous: [shot.position; 2], frozen: true, info, hit_list: Vec::new(), hits: 0, no_clash: true, clashed: false, map_hit: true, clash_list: [TargetId::NONE; 4] };
    if !shot.effect.is_empty() {
        cx.spawn_effect(EffectSpawn { id: EffectId(notify_asset_id(shot.effect)), name: shot.effect, at: Attach::World(shot.position), rotation: Vec3::ZERO, scale: 1.0, persistent: false });
    }
    let combat = &mut cx.pawn.combat;
    combat.shots.push(Shot { name: shot.row, position: shot.position, yaw: 0.0, life: shot.life, size: (shot.radius, shot.grow, shot.max), curve: &[], window, offset: Vec3::ZERO, convex: None, attach: None });
    combat.shots_spawned += 1;
}

/// (XSI, 2026-10-16) A hit sphere of the XS abilities (abilities/xs: Spectral Rays' beams, Light Wave's waves ...):
/// a shot of row `row` (`p_ex001_AttackData`) at `position`, of `radius`, for `life` seconds, with its own hit effect.
/// Not a KH3 actor: the group names (`XS_*`) are the port's.
pub(crate) fn spawn_xs_shot(cx: &mut Ctx, group: &'static str, row: &'static str, position: Vec3, radius: f32, life: f32, hit_effect: Option<&'static str>) {
    let (index, row_name) = attack_row(row).map_or((u32::MAX, row), |(i, r)| (i as u32, r.id));
    let info = AttackCtx { state: false, aerial: false, ..cx.pawn.combat.attack_ctx };
    let window = Window { key: (u16::MAX, u16::MAX), group, row: index, row_name, hit_se: None, hit_se_asset: None, hit_effect, hit_effect_offset: 0.0, interval: 0.0, timer: 0.0, time: 0.0, initialised: true, previous: [position; 2], frozen: true, info, hit_list: Vec::new(), hits: 0, no_clash: true, clashed: false, map_hit: true, clash_list: [TargetId::NONE; 4] };
    let combat = &mut cx.pawn.combat;
    combat.shots.push(Shot { name: group, position, yaw: cx.pawn.yaw, life, size: (radius, 0.0, radius), curve: &[], window, offset: Vec3::ZERO, convex: None, attach: None });
    cx.pawn.combat.shots_spawned += 1;
}

/// Where a shot of the running attack starts: pawn vt+0x1410(index) 0x1405AA790 -> equipment
/// vt+0x580 0x1417812E0 (decompiled C, verdict same-partial): weapon `m_WeaponEquips[index]`
/// (index 0 when out of range), its mesh's socket `MuzzleAttachPoint` (weapon+0x5A0; the class
/// default "ATK_01", constructor 0x141777AE0; the Kingdom Key's blueprint does not change it):
/// the keyblade's TIP socket of the playing clip. Without a weapon: the actor location.
pub(crate) fn shot_origin(cx: &Ctx) -> Vec3 {
    let pawn = &*cx.pawn;
    let (sin, cos) = pawn.yaw.to_radians().sin_cos();
    let world = |v: Vec3| pawn.cmc.location + Vec3::new(v.x * cos - v.y * sin, v.x * sin + v.y * cos, v.z);
    let data = pawn.anim.name.data();
    if let Some(&(_, index)) = data.samples.first() {
        let sequence = &SEQUENCES[index];
        if let Some(last) = sequence.sockets.len().checked_sub(1) {
            let f = if sequence.length > 0.0 { (pawn.anim.time / sequence.length).clamp(0.0, 1.0) * last as f32 } else { 0.0 };
            let (i, t) = ((f.floor() as usize).min(last), f - f.floor());
            let (a, b) = (sequence.sockets[i], sequence.sockets[(i + 1).min(last)]);
            // Samples are [ATK_00 xyz, ATK_01 xyz] in actor space: columns 3 .. 5 are ATK_01.
            let at = |k: usize| a[k] + (b[k] - a[k]) * t;
            return world(Vec3::new(at(3), at(4), at(5)));
        }
    }
    world(Vec3::new(0.0, 0.0, 91.0))
}

fn row_of(window: &Window) -> Option<&'static AttackDataRow> {
    super::tables::ATTACK_DATA.get(window.row as usize)
}

/// The notifies of this pose tick: begin and end of hit windows (pawn interface slots 64 / 65).
pub(crate) fn on_notifies(cx: &mut Ctx, notifies: &Notifies) {
    for event in &notifies.events {
        // `TresAnimNotifyState_AttackEnableMapHitReflect` (0x1405620F0): prim+0x987 bit 1 of the
        // active primitives while it lasts.
        if event.class() == crate::anim::NotifyClass::TresAnimNotifyStateAttackEnableMapHitReflect {
            match event.phase {
                NotifyPhase::Begin => cx.pawn.combat.map_reflect = true,
                NotifyPhase::End => cx.pawn.combat.map_reflect = false,
                _ => {}
            }
            continue;
        }
        let NotifyData::Attack { group, row, hit_se, interval, hit_se_asset, override_hit_effect, hit_effect_group } = event.def().data else { continue };
        // (party stream D6b) m_bOverrideHitEffect + m_HitEffectGrpName: the chooser 0x140628960 takes
        // the m_HitEffects entry of that group. FUSION_ROCKET (GooFRocket 1765, window SphereM): the
        // entry of p_ex001_PawnBase TresAtkColl0 (vn_ex002_frt_hit0, no sound) [A]; that the pawn's
        // component answers for the keyblade's SphereM is [I] (sheet sora-effects). Other groups: not ported.
        let group_effect = match (override_hit_effect, hit_effect_group) {
            (true, Some("FUSION_ROCKET")) => Some("vn_ex002_frt_hit0"),
            _ => None,
        };
        let key = (event.sequence, event.index);
        match event.phase {
            NotifyPhase::Begin => {
                let info = cx.pawn.combat.attack_ctx;
                // The pre-processor's row rename: kind 0x0E turns KB_DOWN into KB_DOWN_FIN.
                let name = if info.kind == 0x0E && row.eq_ignore_ascii_case("KB_DOWN") { "KB_DOWN_FIN" } else { row };
                let (index, row_name) = attack_row(name).map_or((u32::MAX, name), |(i, r)| (i as u32, r.id));
                // `m_bDisableLocation1Attach`: the position is frozen after the first tick.
                let frozen = matches!(group, "KB_FixRangeS" | "KB_FixPillar" | "SpreadS" | "ImpactLL" | "ImpactM");
                // Row -> primitive (slot 327): bit 7 = reflect level <= 0; the activation's S+0x5C
                // bit 1 (combo >= 1) sets it again and zeroes the level.
                let level = super::tables::ATTACK_DATA.get(index as usize).map_or(0, |r| r.reflect_level);
                let no_clash = level <= 0 || info.counters[0] >= 1;
                let window = Window { key, group, row: index, row_name, hit_se, hit_se_asset, hit_effect: group_effect, hit_effect_offset: 0.0, interval: interval.unwrap_or(0.0), timer: 0.0, time: 0.0, initialised: false, previous: [Vec3::ZERO; 2], frozen, info, hit_list: Vec::new(), hits: 0, no_clash, clashed: false, map_hit: false, clash_list: [TargetId::NONE; 4] };
                // An activation of a group replaces its record (the lists are cleared).
                cx.pawn.combat.windows.retain(|w| w.group != group);
                cx.pawn.combat.windows.push(window);
            }
            NotifyPhase::End => cx.pawn.combat.windows.retain(|w| w.key != key),
            _ => {}
        }
    }
}

/// Closest points of two segments: (point on the first, point on the second).
fn closest(p1: Vec3, q1: Vec3, p2: Vec3, q2: Vec3) -> (Vec3, Vec3) {
    let (d1, d2, r) = (q1 - p1, q2 - p2, p1 - p2);
    let (a, e, f) = (d1.length_squared(), d2.length_squared(), d2.dot(r));
    let (s, t);
    if a <= 1.0e-8 && e <= 1.0e-8 {
        return (p1, p2);
    }
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

/// The shape of a window now: (end point 1, end point 2, radius), world. `None`: the group has
/// no keyblade shape this port knows how to place.
fn shape(cx: &Ctx, window: &Window) -> Option<(Vec3, Vec3, f32)> {
    // (XSI) The XS abilities' groups that the Kingdom Key's component lacks (abilities/xs: Ancient Light's pillars).
    let Some(row) = group_shapes(window.group).next() else { return crate::abilities::xs::group_shape(cx, window.group, window.info.target) };
    let pawn = &*cx.pawn;
    let (sin, cos) = pawn.yaw.to_radians().sin_cos();
    let world = |v: Vec3| pawn.cmc.location + Vec3::new(v.x * cos - v.y * sin, v.x * sin + v.y * cos, v.z);
    // The sockets of the keyblade at the clip's current position (the clip's own pose).
    let sequence = &SEQUENCES[window.key.0 as usize];
    let sockets = || -> Option<(Vec3, Vec3)> {
        let track = sequence.sockets;
        let last = track.len().checked_sub(1)?;
        let playing = pawn.anim.name.data().samples.iter().any(|&(_, s)| s == window.key.0 as usize);
        let time = if playing { pawn.anim.time } else { sequence.length };
        let f = if sequence.length > 0.0 { (time / sequence.length).clamp(0.0, 1.0) * last as f32 } else { 0.0 };
        let (i, t) = ((f.floor() as usize).min(last), f - f.floor());
        let (a, b) = (track[i], track[(i + 1).min(last)]);
        let at = |k: usize| a[k] + (b[k] - a[k]) * t;
        Some((Vec3::new(at(0), at(1), at(2)), Vec3::new(at(3), at(4), at(5))))
    };
    // Size over time (0x140649E40; C 8.1): the two growing shapes of the base move set.
    let t = window.time;
    // (`m_IncSize` / `m_IncMaxSize` are not columns of the sheet: the growing groups of the
    // keyblade component, from its asset.)
    let radius = match window.group {
        "FrontImpactS" | "SpreadS" => (100.0 + 700.0 * t).min(350.0),
        "ImpactLL" => (100.0 + 2800.0 * t).min(1400.0),
        "ImpactM" => (100.0 + 1100.0 * t).min(550.0),
        "KB_FixPillar" => (50.0 + 2000.0 * t).min(300.0),
        _ => row.size_x,
    };
    let owner_socket = |name: &str| match name {
        "vfx_cure" => Some(if row.loc1.first() == Some(&200.0) { Vec3::from(SOCKET_VFX_CURE_PLUS_200) } else { Vec3::from(SOCKET_VFX_CURE) }),
        // The body mesh's `center` socket (hip height): without a track of its own the capsule
        // centre stands in (TODO(combat): tracks for center, R_te, kh_sk ...).
        "center" => Some(Vec3::new(0.0, 0.0, 91.0)),
        _ => None,
    };
    match (row.shape, row.socket1) {
        ("CAPSULE2", "ATK_00") if row.socket2 == "ATK_01" => {
            let (a, b) = sockets()?;
            Some((world(a), world(b), radius))
        }
        ("SPHERE", "ATK_00") => {
            let (a, _) = sockets()?;
            Some((world(a), world(a), radius))
        }
        ("SPHERE", "ATK_01") => {
            let (_, b) = sockets()?;
            Some((world(b), world(b), radius))
        }
        ("SPHERE", socket) => owner_socket(socket).map(|p| (world(p), world(p), radius)),
        // CAPSULE1 with rotation LOCAL: an upright pillar of half-length `m_Size.Z`.
        ("CAPSULE1", socket) => owner_socket(socket).map(|p| (world(p) - Vec3::Z * row.size_z, world(p) + Vec3::Z * row.size_z, radius)),
        _ => None,
    }
}

/// The knockback direction (`KnockbackDir` 0x14061EA30; C 8.7): for Sora's rows (NORMAL and
/// INSTIGATOR_PAWN) the 3D unit vector from his root to the victim's root; zero when they
/// coincide.
pub fn knockback_direction(sora_root: Vec3, victim_root: Vec3) -> Vec3 {
    let d = victim_root - sora_root;
    if d.length_squared() < 1.0e-8 {
        Vec3::ZERO
    } else {
        d.normalize()
    }
}

/// The damage of one hit of row `row` (None: the static default row, power 1) with the damage
/// formula of the status implementer (`status::compute_damage`, spec 8.3). `combo_step` is the
/// info's third counter (Attack state +0x130: C 8.7).
pub(crate) fn damage(cx: &Ctx, target: TargetId, row: Option<&AttackDataRow>, aerial: bool, combo_step: u8, just_guard: bool) -> f32 {
    let status = &cx.pawn.status;
    // (XSI) Hidden Potential's floor against a powerful foe (abilities/xs).
    let (strength, magic) = crate::abilities::xs::overcome_stats(cx, target, status.stats.strength, status.stats.magic);
    let attacker = Attacker { has_status: true, strength, magic, abilities: Some(&status.abilities), side: Side::Player, state_boost: false };
    let defender = Defender::of_target(cx.host.targets.defense(target).as_ref());
    let attack = DamageAttack {
        aerial,
        just_guard,
        counter: row.is_some_and(|r| r.counter),
        second_resist: row.is_some_and(|r| r.rapid_fire),
        combo_step: combo_step.min(99) as i8,
        attribute: row.map_or(ETresDamageAttribute::Physical, |r| r.attribute),
        magic: row.is_some_and(|r| r.magic),
        recover: row.is_some_and(|r| r.damage_kind == ETresDamageKind::Recover),
        ..DamageAttack::physical(row.map_or(1.0, |r| r.power))
    };
    compute_damage(&attacker, &defender, &attack, &DamageContext::of(status)).amount
}

/// One tick of every active window (the attack-collision manager's passes 1 and 2).
pub(crate) fn tick(cx: &mut Ctx, dt: f32) {
    let mut k = 0;
    while k < cx.pawn.combat.windows.len() {
        let mut window = cx.pawn.combat.windows[k].clone();
        window.time += dt;
        if window.clashed {
            // rec flag bit 1 (C 8.2.2 pass 1): no interval -> the group is deactivated now;
            // else it stops sweeping until the interval elapses.
            if window.interval <= 0.0 {
                cx.pawn.combat.windows.remove(k);
                continue;
            }
        } else if let Some((a, b, radius)) = shape(cx, &window) {
            let (a, b) = if window.frozen && window.initialised { (window.previous[0], window.previous[1]) } else { (a, b) };
            // The first tick of an activation tests the shape in place (C 8.2.3).
            let from = if window.initialised { window.previous } else { [a, b] };
            window.previous = [a, b];
            window.initialised = true;
            sweep(cx, &mut window, from, [a, b], radius);
        }
        // TimerTick (0x14062CFC0; C 8.2.2): the clock starts at the first hit; when it elapses
        // the whole hit list is emptied.
        if window.interval > 0.0 {
            if window.hit_list.is_empty() && !window.clashed {
                window.timer = 0.0;
            } else {
                window.timer += dt;
                if window.timer >= window.interval {
                    window.hit_list.clear();
                    window.timer = 0.0;
                    if window.clashed {
                        window.clashed = false;
                        window.clash_list = [TargetId::NONE; 4];
                    }
                }
            }
        }
        cx.pawn.combat.windows[k] = window;
        k += 1;
    }
    // The shots: each a stationary growing sphere (or, D6, a CONVEX prism) at the actor + the
    // shape's relative location for its life; every target once.
    let mut k = 0;
    while k < cx.pawn.combat.shots.len() {
        let mut shot = cx.pawn.combat.shots[k].clone();
        shot.window.time += dt;
        shot.life -= dt;
        // (D6b) An attached actor follows its friend's actor transform (KeepWorldTransform at the spawn).
        if let Some(a) = shot.attach {
            if let Some(f) = cx.pawn.combat.friend.friend(a.uid) {
                shot.position = f.position + yawed(a.local, f.yaw);
                shot.yaw = f.yaw + a.yaw;
            }
        }
        let centre = shot.centre();
        if let Some(prism) = shot.prism() {
            let axis = [centre - Vec3::Z * prism.half_height, centre + Vec3::Z * prism.half_height];
            sweep_shape(cx, &mut shot.window, axis, axis, prism.radius.x.max(prism.radius.y), Some(&prism));
        } else {
            // Swept from the shape's previous centre (a moving, attached shot); a static one tests in place.
            let (from, radius) = (shot.window.previous[0], shot.radius());
            shot.window.previous = [centre; 2];
            sweep(cx, &mut shot.window, [from; 2], [centre; 2], radius);
        }
        if shot.life <= 0.0 {
            cx.pawn.combat.shots.remove(k);
        } else {
            cx.pawn.combat.shots[k] = shot;
            k += 1;
        }
    }
}

const SWEEP_SAMPLES: usize = 4;

/// The attack-collision manager's common effects (`ATresAtkCollManager` +0x400 ... +0x428, from
/// `TresGameModeLoadAsset` [A `Load/Common/TresGameModeLoad`]; which one `HitEffects`
/// 0x1406294F0 spawns: decompiled C): a guarded hit / a clash that guards `m_CmnGuardReflectEffect`
/// (0x140646E20, results 4 / 5), a clash that staggers `m_CmnStaggerReflectEffect` (result 3),
/// a blocked or armoured hit `m_CmnInvincibleHitEffect` (0x140646D60).
pub const EFFECT_GUARD_REFLECT: &str = "vs_com_haj00";
pub const EFFECT_STAGGER_REFLECT: &str = "vs_com_yor00";
pub const EFFECT_NO_DAMAGE: &str = "vs_com_mtk0";

/// The clash pass (C 8.2.4): only when the prim may clash. Returns true when it clashed.
fn clash_pass(cx: &mut Ctx, window: &mut Window, to: [Vec3; 2], radius: f32) -> bool {
    if window.no_clash {
        return false;
    }
    let mut shapes: Vec<AttackShape> = Vec::new();
    cx.host.targets.attack_shapes(&mut shapes);
    let row = row_of(window);
    let own_level = row.map_or(0, |r| r.reflect_level);
    let mut clashed = false;
    for other in shapes {
        if other.reflect_level <= 0 || window.clash_list.contains(&other.owner) || window.hit_list.contains(&other.owner) {
            continue;
        }
        let (p, q) = closest(to[0], to[1], other.a, other.b);
        if (p - q).length() > radius + other.radius {
            continue;
        }
        // 0x14061920E - 0x1406192F2: REFLECT when the other's level is higher, else GUARD_REFLECT;
        // then my row's m_ReflectReaction.
        let mut result = if other.reflect_level as i32 - own_level > 0 { ReactionResult::Reflect } else { ReactionResult::GuardReflect };
        match row.map_or("DEFAULT", |r| r.reflect_reaction) {
            "STAGGER" => result = ReactionResult::Reflect,
            "GUARD" => result = ReactionResult::GuardReflect,
            "NOREACTION" => result = ReactionResult::None,
            _ => {}
        }
        window.clashed = true;
        if let Some(slot) = window.clash_list.iter_mut().find(|t| **t == TargetId::NONE) {
            *slot = other.owner;
        }
        let point = (p + q) * 0.5;
        // The manager's queued clash effect (pass 3 0x140640F70, decompiled C): result REFLECT ->
        // `m_CmnStaggerReflectEffect` (`vs_com_yor00`), GUARD_REFLECT / GUARD ->
        // `m_CmnGuardReflectEffect` (`vs_com_haj00`) [A TresGameModeLoad], at the contact.
        if result != ReactionResult::None {
            let name = if result == ReactionResult::Reflect { EFFECT_STAGGER_REFLECT } else { EFFECT_GUARD_REFLECT };
            cx.spawn_effect(EffectSpawn { id: EffectId(notify_asset_id(name)), name, at: Attach::World(point), rotation: Vec3::ZERO, scale: 1.0, persistent: false });
        }
        cx.pawn.event(EventKind::Clash { target: other.owner, result });
        cx.pawn.combat.clashes += 1;
        let position = cx.target(other.owner).map_or(q, |t| t.position);
        on_contact(cx, window, result, other.owner, position, point, None);
        clashed = true;
    }
    clashed
}

/// The map part of the sweep (C 8.2.3 response set: WorldStatic only with `m_MapHitType` >=
/// HITMAP; C 8.2.5 step 9: the map gate), one hit per activation.
fn map_pass(cx: &mut Ctx, window: &mut Window, from: [Vec3; 2], to: [Vec3; 2], radius: f32) {
    let Some(shape) = group_shapes(window.group).next() else { return };
    // The map is "in the hit list" after its first hit - except while the reflect window is open
    // (step 10: `reflectMap`: the hit is classified and reported again, without effects).
    let reflect = cx.pawn.combat.map_reflect;
    if (window.map_hit && !reflect) || !matches!(shape.map_hit, "IGNORE_GROUND" | "HITMAP") {
        return;
    }
    let mut best: Option<crate::world::Hit> = None;
    for k in 0..3 {
        let f = k as f32 * 0.5;
        let (start, end) = (from[0].lerp(from[1], f), to[0].lerp(to[1], f));
        if let Some(hit) = cx.world.sweep_capsule(start, end, radius, radius) {
            if best.map_or(true, |b| hit.time < b.time) {
                best = Some(hit);
            }
        }
    }
    let Some(hit) = best else { return };
    // The map gate (step 9): the owner's notify slot 24 answers for a running attack state
    // (0x141342760, `attack_map_hit` below): 1 = not a wall worth hitting, 2 = hit it. Without an
    // attack state the answer is 0 and the component's own two tests apply.
    if window.info.state {
        if attack_map_hit(cx, &hit, shape.phys, reflect, to[1]) == 1 {
            return;
        }
    } else {
        // Floor-like surfaces are ignored ...
        if shape.map_hit == "IGNORE_GROUND" && hit.impact_normal.z >= P.hit_map_floor_cos {
            return;
        }
        // ... and a physical shape ignores contacts that are not above his root + 50 cm.
        if shape.phys && cx.pawn.cmc.location.z + P.hit_weapon_map_height_limit >= hit.position.z {
            return;
        }
    }
    let again = std::mem::replace(&mut window.map_hit, true);
    // Classify (8.2.6, the rows without a hurt-shape interface): with the reflect window open the
    // level repels the swing when there is no actor behind the surface or the actor's default
    // response is GUARD / REFLECT; else V = DAMAGE, Att = NOREACTION. A null actor gets no
    // TakeDamage.
    let response = if reflect { cx.world.attack_hit_response(hit.component) } else { Some(0) };
    let attacker = if reflect && matches!(response, None | Some(1) | Some(2)) { ReactionResult::Reflect } else { ReactionResult::NoReaction };
    if !again {
        cx.pawn.combat.wall_hits += 1;
        // The chooser on a map hit: `VW_SO010_WHT0` (`m_bChangeMapHit`), no sound. (Not for the
        // repeated hit of the reflect window: `reflectMap`.)
        cx.spawn_effect(EffectSpawn { id: EffectId(NOTIFY_ID_NONE), name: "VW_SO010_WHT0", at: Attach::World(hit.position), rotation: normal_rotation(hit.impact_normal) /* (vfx implementer 2) */, scale: 1.0, persistent: false });
        cx.pawn.event(EventKind::WallHit { position: hit.position, normal: hit.impact_normal });
    } else if attacker != ReactionResult::Reflect {
        return;
    }
    on_contact(cx, window, attacker, TargetId::NONE, hit.position, hit.position, Some(hit.impact_normal));
}

/// On contact (owner notify slot 15 0x14140B8A0 -> the Attack state's 0x141341B10; C 8.5.1).
fn on_contact(cx: &mut Ctx, window: &Window, result: ReactionResult, victim: TargetId, victim_position: Vec3, point: Vec3, map: Option<Vec3>) {
    let mut result = result;
    // Kinds 0x0A / 0x19 with row KB_FLASHSTEP_END: never repelled.
    if matches!(window.info.kind, 0x0A | 0x19) && window.row_name.eq_ignore_ascii_case("KB_FLASHSTEP_END") {
        result = ReactionResult::NoReaction;
    }
    let combat = &mut cx.pawn.combat;
    combat.last_impact = point;
    if matches!(result, ReactionResult::Reflect | ReactionResult::GuardReflect | ReactionResult::Guard) {
        // The recoil record (state+0x264 |= 1, +0x268 = result, +0x270 = the victim). A map hit
        // only with combo < 1, no target and the state's slot 140 (`map_recoil` below): the
        // record is REFLECT without a causer.
        match map {
            None => {
                if window.info.state {
                    combat.recoil = Some((result, victim, victim_position));
                }
            }
            Some(normal) => {
                if window.info.state && window.info.counters[0] < 1 && !window.info.has_target && map_recoil(cx.pawn.cmc.location, cx.pawn.yaw, point, normal) {
                    let combat = &mut cx.pawn.combat;
                    combat.recoil = Some((ReactionResult::Reflect, TargetId::NONE, point));
                    combat.map_recoils += 1;
                }
            }
        }
    } else {
        // pawn+0xE24 += 1; pawn+0xE28 = 1; state+0x13C |= 4; state+0x234 |= 0x2000. (A wall hit
        // that is not repelled takes this branch too: decompiled C.)
        combat.hit_count += 1;
        combat.hit_flag = true;
        combat.swing_hit = true;
        // Final Blow (0x141341F5F - 0x14134208A, decompiled C; spec 5.6.4): a guard counter (kinds
        // 0x21, 0x23, 0x24) that hits the state's target pawn, with REVENGE_EX (7):
        // m_pRevengePawn = it; the chase is armed while its reaction (its request interface
        // slot 89) is BLOW / SIDESPIN / VERTICALROLL (2..4); armed: the reaction command 12
        // (REVENGESLASH2) is shown for 1.0 s (state slot 141 -> 0x141414350).
        if map.is_none() && window.info.state && matches!(window.info.kind, 0x21 | 0x23 | 0x24) && victim != TargetId::NONE && victim == window.info.target && cx.pawn.status.abilities.has(crate::enums::ETresAbilityKind::RevengeEx) {
            let armed = chase_ready(cx, victim);
            let combat = &mut cx.pawn.combat;
            combat.revenge = victim;
            if armed && !combat.chase_armed {
                combat.chase_armed = true;
                super::situation::set_reaction(cx, P.chase_command as i32, P.chase_prompt_time);
            }
        }
    }
}

/// The Attack state's map-hit handler (owner notify slot 24, 0x141342760; read in decompiled C,
/// the thresholds are the constants of that function): what a contact with the level is to a
/// running attack. 1 = ignored (floor-like, or low for a physical shape), 2 = a wall worth
/// hitting.
///
/// ```text
/// n = dot(Up, hit.ImpactNormal);  low = hit.ImpactPoint.z <= root.z + 45
/// if recoil record not set && prim reflect window (prim+0x987 bit 1) && (n >= 0.656059 || n <= -0.8660254):
///     trace "ReflectCheck" from root + Up x 50 to the primitive (CAPSULE2: its end nearer to that
///     start), first blocking hit h:
///         m = dot(Up, h.ImpactNormal)
///         return (m <= -0.8660254 || m >= 0.656059 || (phys && low && h.ImpactPoint.z <= root.z + 45)) ? 1 : 2
/// if n < 0.656059: return (phys && low) ? 1 : 2
/// return 1
/// ```
///
/// So floors (up to 49 degrees of slope) never count; a floor- or ceiling-like contact during the
/// reflect window is re-examined along the line from his chest to the blade (a wall between the
/// two makes it a wall hit after all); and a physical shape (the keyblade) ignores walls lower
/// than 45 cm above his root.
fn attack_map_hit(cx: &Ctx, hit: &crate::world::Hit, phys: bool, reflect_window: bool, blade: Vec3) -> u8 {
    const FLOOR: f32 = 0.656059;
    const CEILING: f32 = -0.866_025_4;
    const LOW: f32 = 45.0;
    let root = cx.pawn.cmc.location;
    let n = hit.impact_normal.z;
    let low = hit.position.z <= root.z + LOW;
    if cx.pawn.combat.recoil.is_none() && reflect_window && (n >= FLOOR || n <= CEILING) {
        if let Some(h) = cx.world.line_trace(root + Vec3::Z * 50.0, blade) {
            let m = h.impact_normal.z;
            return if m <= CEILING || m >= FLOOR || (phys && low && h.position.z <= root.z + LOW) { 1 } else { 2 };
        }
    }
    if n < FLOOR {
        if phys && low { 1 } else { 2 }
    } else {
        1
    }
}

/// Attack slot 113 (0x1413325A0 [I]): the revenge pawn's reaction (its request interface slot 89:
/// the damage kind of the reaction it is in) is 2, 3 or 4.
pub(crate) fn chase_ready(cx: &Ctx, pawn: TargetId) -> bool {
    pawn != TargetId::NONE && matches!(cx.host.targets.reaction_state(pawn), 2..=4)
}

/// The map-hit recoil test (Attack slot 140, 0x14133D4D0; decompiled C, constants from it): the
/// impact point in the actor's frame, taken from 50 cm above his root, must lie within 80
/// degrees of his facing in yaw and pitch; above 40 degrees of pitch a ceiling-like surface
/// (dot(Up, n) <= -0.8660254) does not repel.
pub fn map_recoil(root: Vec3, yaw: f32, impact: Vec3, normal: Vec3) -> bool {
    let d = impact - root;
    let (sin, cos) = yaw.to_radians().sin_cos();
    let local = Vec3::new(d.x * cos + d.y * sin, -d.x * sin + d.y * cos, d.z - 50.0);
    if local.length_squared() < 1.0e-8 {
        return true;
    }
    let pitch = local.z.atan2(local.truncate().length()).to_degrees();
    let yaw = local.y.atan2(local.x).to_degrees();
    if yaw.abs() < 80.0 && pitch.abs() < 80.0 {
        return pitch < 40.0 || normal.z > -0.866_025_4;
    }
    false
}

fn sweep(cx: &mut Ctx, window: &mut Window, from: [Vec3; 2], to: [Vec3; 2], radius: f32) {
    sweep_shape(cx, window, from, to, radius, None);
}

/// (party stream D6) The overlap of a hurt capsule (`a`..`b`, radius `r`) with a CONVEX prism:
/// (contact point on the hurt surface, normal from the hurt shape toward the prism).
fn prism_contact(prism: &Prism, a: Vec3, b: Vec3, r: f32) -> Option<(Vec3, Vec3)> {
    let on_hurt = prism.nearest_on_segment(a, b);
    let gap = prism.closest_point(on_hurt) - on_hurt;
    if gap.length() > r {
        return None;
    }
    // Inside the prism: toward its axis (the impact faces the column's centre).
    let normal = if gap.length_squared() > 1.0e-4 { gap.normalize() } else { ((prism.centre - on_hurt) * Vec3::new(1.0, 1.0, 0.0)).try_normalize().unwrap_or(Vec3::Z) };
    Some((on_hurt + normal * r, normal))
}

fn sweep_shape(cx: &mut Ctx, window: &mut Window, from: [Vec3; 2], to: [Vec3; 2], radius: f32, prism: Option<&Prism>) {
    if clash_pass(cx, window, to, radius) {
        // (Hits on attack shapes are dropped; the record stops sweeping from the next tick.)
    }
    map_pass(cx, window, from, to, radius);
    let mut targets: Vec<TargetInfo> = Vec::new();
    cx.targets(&mut targets);
    // (time, target, part, contact point, normal), earliest contact per target.
    let mut hits: Vec<(f32, TargetInfo, u32, Vec3, Vec3)> = Vec::new();
    let samples = if from == to { 0 } else { SWEEP_SAMPLES };
    let mut shapes: Vec<HurtShape> = Vec::new();
    for target in &targets {
        if !target.flags.has(TargetFlags::ALIVE | TargetFlags::HITTABLE) || window.hit_list.contains(&target.id) {
            continue;
        }
        shapes.clear();
        cx.hurt_shapes(target.id, &mut shapes);
        'target: for step in 0..=samples {
            let t = if samples == 0 { 0.0 } else { step as f32 / samples as f32 };
            let (a, b) = (from[0].lerp(to[0], t), from[1].lerp(to[1], t));
            for hurt in &shapes {
                if let Some(prism) = prism {
                    // (D6) A static prism (no sweep: shots do not move).
                    if let Some((point, normal)) = prism_contact(prism, hurt.a, hurt.b, hurt.radius) {
                        hits.push((t, *target, hurt.part, point, normal));
                        break 'target;
                    }
                    continue;
                }
                let (on_shape, on_hurt) = closest(a, b, hurt.a, hurt.b);
                let gap = on_shape - on_hurt;
                if gap.length() <= radius + hurt.radius {
                    let normal = if gap.length_squared() > 1.0e-8 { gap.normalize() } else { Vec3::Z };
                    hits.push((t, *target, hurt.part, on_hurt + normal * hurt.radius, normal));
                    break 'target;
                }
            }
        }
    }
    // The sort of `Apply` (0x1406217F0 + 0x140614B80, decompiled C; spec 8.2.4): by hit Time
    // ascending; ties by the distance of the impact point from lerp(ref1, ref2, Time), the
    // reference by the row's knockback type: ATK_COLLISION / VELOCITY the shape's path (previous
    // centre -> centre), INSTIGATOR_PAWN and NORMAL the instigator's location (Sora's root).
    // (Attack-shape components sort first: the clash pass above has taken them already.)
    let by_path = matches!(row_of(window).map_or("NORMAL", |r| r.knockback), "ATK_COLLISION" | "VELOCITY");
    let (ref1, ref2) = if by_path { ((from[0] + from[1]) * 0.5, (to[0] + to[1]) * 0.5) } else { (cx.pawn.cmc.location, cx.pawn.cmc.location) };
    let key = |h: &(f32, TargetInfo, u32, Vec3, Vec3)| (h.3 - ref1.lerp(ref2, h.0)).length_squared();
    hits.sort_by(|x, y| x.0.total_cmp(&y.0).then(key(x).total_cmp(&key(y))));
    for (time, target, part, point, normal) in hits {
        apply(cx, window, &target, part, point, normal, time);
    }
}

/// (vfx implementer 2) The rotation of a hit effect: the rotator whose X axis is the hit normal
/// (stock `FVector::Rotation` [U]: yaw = atan2(y, x), pitch = atan2(z, |xy|), roll 0), in the
/// seam's order (x = pitch, y = yaw, z = roll; degrees). A zero normal gives no rotation.
pub fn normal_rotation(normal: Vec3) -> Vec3 {
    if normal.length_squared() < 1.0e-8 {
        return Vec3::ZERO;
    }
    Vec3::new(normal.z.atan2(normal.truncate().length()).to_degrees(), normal.y.atan2(normal.x).to_degrees(), 0.0)
}

/// One hit (C 8.2.5 steps 16 - 21 for a DEFAULT body).
fn apply(cx: &mut Ctx, window: &mut Window, target: &TargetInfo, part: u32, point: Vec3, normal: Vec3, time: f32) {
    let row = row_of(window);
    let counters = window.info.counters.map(|c| c.clamp(0, 99) as u8);
    let mut info = HitInfo { counters, impact_normal: normal, time, ..HitInfo::DEFAULT };
    if window.info.just_guard {
        info.flags51 |= HitInfo::FLAG51_JUST_GUARD;
    }
    if window.info.finish_end {
        info.flags50 |= HitInfo::FLAG50_FINISH;
    }
    if window.info.aerial {
        info.flags50 |= HitInfo::FLAG50_AERIAL;
    }
    if let Some(r) = row {
        info = HitInfo {
            power: r.power,
            power_scale: r.power_scale,
            parabola_angle: r.parabola_angle,
            move_length: r.move_length,
            brake: r.brake,
            effect_time: r.effect_time,
            knockback: r.knockback,
            bad_status: r.bad_status,
            bad_time: r.bad_time,
            bad_param: r.bad_param,
            reaction_power: r.reaction_power,
            armor_attack_power: r.armor_attack,
            form_point: r.form_point,
            revenge_point: r.revenge_point,
            phys_force: r.phys_force,
            // The reflect level is forced to 0 after the first swing of a combo (C 8.2.1).
            reflect_level: if window.info.counters[0] >= 1 { 0 } else { r.reflect_level as i8 },
            reflect_reaction: r.reflect_reaction,
            ..info
        };
        info.flags50 |= (r.magic as u8) | (r.killer as u8) << 2 | (r.invalid_guard as u8) << 3 | (r.rapid_fire as u8) << 4;
        info.flags51 |= (r.attraction as u8) | (r.counter as u8) << 1 | (r.weak_guard as u8) << 3;
    }
    // Step 18: the bad-status roll against `m_BadStatusGenRate` (uniform [0, 100); the engine's
    // xorshift128 is a process global: here the pawn's own stream; never drawn for 0 or 100).
    if let Some(r) = row.filter(|r| r.bad_status != "NONE" && r.bad_gen_rate < 100.0) {
        let rng = &mut cx.pawn.combat.rng;
        if *rng == 0 {
            *rng = 0x9E37_79B9;
        }
        *rng ^= *rng << 13;
        *rng ^= *rng >> 17;
        *rng ^= *rng << 5;
        let roll = (*rng >> 8) as f32 * P.hit_bad_status_roll_scale;
        if r.bad_gen_rate <= 0.0 || roll > r.bad_gen_rate {
            info.clear_bad_status();
        }
    }
    // (XSI) The XS abilities' changes of the reaction power (abilities/xs `pre_hit`).
    crate::abilities::xs::pre_hit(cx, window, target.id, &mut info);
    let attribute = row.map_or(ETresDamageAttribute::Physical, |r| r.attribute);
    let kind = row.map_or(ETresDamageKind::Small, |r| r.damage_kind);
    // Classify (0x14061FE20; C 8.2.6): the victim's answers come from the host.
    let body = cx.host.targets.body(target.id, part);
    let shape_flags = Shape { no_clash: window.no_clash, victim_may_refuse: false, reflect_window: false, p7: false };
    let id = target.id;
    let (mut armor, mut endurance) = (None, None);
    let c = classify(&body, MapResponse::Other, &shape_flags, kind, attribute, &mut info, &mut |power, weak| {
        armor = Some((power, weak));
        // (Answered below: the closure cannot reach the host while `info` is borrowed.)
        i32::MIN
    }, &mut |power| {
        endurance = Some(power);
        i32::MIN
    });
    // The two victim-changing calls, then the classification again with their real answers.
    let c = if armor.is_some() || endurance.is_some() {
        let armor_left = armor.map(|(power, weak)| cx.host.targets.take_armor(id, power, weak));
        let endurance_left = endurance.map(|power| cx.host.targets.take_reaction_power(id, power));
        info.flags50 &= !HitInfo::FLAG50_ARMOR_DAMAGE;
        classify(&body, MapResponse::Other, &shape_flags, kind, attribute, &mut info, &mut |_, _| armor_left.unwrap_or(0), &mut |_| endurance_left.unwrap_or(0))
    } else {
        c
    };
    let amount = damage(cx, target.id, row, window.info.aerial, counters[2], window.info.just_guard);
    // (XSI) The XS abilities' changes of the amount (the dive bonus, the refraction share, the spectacle cap).
    let amount = crate::abilities::xs::adjust_damage(cx, window, target, amount);
    let direction = knockback_direction(cx.pawn.cmc.location, target.position);
    // The victim is recorded (step 19) unless the classification says no (a ghost).
    if c.recorded {
        window.hit_list.push(target.id);
    }
    if c.attacker == ReactionResult::None {
        // result == 0: no effect, no TakeDamage, no on-contact (steps 20 / 21).
        return;
    }
    window.hits += 1;
    // Step 20: effect and sound (`HitEffects` 0x1406294F0, chooser 0x140628960; C 8.6, the
    // Kingdom Key's `m_HitEffects` entry `None`).
    let phys = group_shapes(window.group).next().is_some_and(|s| s.phys);
    let effect = if let Some(own) = window.hit_effect { own } else { match attribute {
        ETresDamageAttribute::Physical => "vw_so010_hit0",
        ETresDamageAttribute::Fire => "vs_fir_hit0",
        ETresDamageAttribute::Blizzard => "vs_blz_hit0",
        ETresDamageAttribute::Thunder => "vs_thd_hit0",
        ETresDamageAttribute::Water => "vs_wat_hit0",
        ETresDamageAttribute::Aero => "vs_win_hit0",
        ETresDamageAttribute::Dark => "vs_dak_hit1",
        _ => "VS_non_HIT0",
    } };
    // (vfx implementer 2) `rot = Rotation(hit.Normal)` (chooser 0x140628960, docs/sora-vfx-spec.md 2.6).
    let rotation = normal_rotation(normal);
    let spawn_at = |cx: &mut Ctx, name: &'static str, at: Vec3| {
        cx.spawn_effect(EffectSpawn { id: EffectId(NOTIFY_ID_NONE), name, at: Attach::World(at), rotation, scale: 1.0, persistent: false });
    };
    let spawn = |cx: &mut Ctx, name: &'static str| spawn_at(cx, name, point);
    // `HitEffects` (0x1406294F0; C 8.6): a blocked / armoured hit and a guarded hit get the
    // victim's (or the common) effect instead of the chooser's and no sound from the chooser.
    if c.victim == ReactionResult::None && c.no_react {
        spawn(cx, EFFECT_NO_DAMAGE);
    } else if matches!(c.victim, ReactionResult::GuardReflect | ReactionResult::Guard) {
        spawn(cx, EFFECT_GUARD_REFLECT);
    } else {
        // (party stream D6b) The chooser 0x140628960: with m_fHitEffectRandomOffsetRadius > 0 the
        // effect / sound position is the impact + FRotator(r1, r2, r3).RotateVector((R, 0, 0))
        // (FUN_142D8D890), each r = (xorshift >> 8) x 2.1457672e-05 - 180 deg (the engine's global
        // stream FUN_142CFF460; here the pawn's own, as the bad-status roll): R x the forward
        // vector of (pitch r1, yaw r2).
        let point = if window.hit_effect_offset > 0.0 {
            let mut roll = || {
                let rng = &mut cx.pawn.combat.rng;
                if *rng == 0 {
                    *rng = 0x9E37_79B9;
                }
                *rng ^= *rng << 13;
                *rng ^= *rng >> 17;
                *rng ^= *rng << 5;
                ((*rng >> 8) as f32 * 2.145_767_2e-5 - 180.0).to_radians()
            };
            let (pitch, yaw) = (roll(), roll());
            let _roll = roll();
            point + Vec3::new(pitch.cos() * yaw.cos(), pitch.cos() * yaw.sin(), pitch.sin()) * window.hit_effect_offset
        } else {
            point
        };
        spawn_at(cx, effect, point);
        if phys {
            spawn_at(cx, "vw_so010_str0", point);
        }
        // The window's hit sound: the notify's asset, else the weapon's alias (HIT1 - 7), else
        // none; no sound when the attacker's result is REFLECT.
        if c.attacker != ReactionResult::Reflect {
            if let Some(name) = window.hit_se_asset.or(window.hit_se) {
                cx.play_sound(SoundPlay { id: SoundId(notify_asset_id(name)), name, at: Attach::World(point), volume: 1.0, pitch: 1.0, looping: false, spatial: true });
            }
        }
    }
    // `TakeDamage` (never with a zero amount: the formula returns 0.1 for 0).
    let record = HitRecord {
        target: target.id,
        part,
        attack: window.row,
        attack_name: window.row_name,
        position: point,
        direction,
        damage: amount,
        damage_kind: kind,
        attribute,
        impulse: Vec3::ZERO,
        // (enemy-reaction implementer) Sora as the victim's damage state reads the instigator.
        attacker: crate::host::HitAttacker { position: cx.pawn.cmc.location, yaw: cx.pawn.yaw, radius: crate::params::SORA_PARAMS.capsule_radius, valid: true },
        // (XSI) the XS abilities' marks for the victim's side (abilities/xs: Break Boost)
        flags: crate::abilities::xs::record_flags(cx, window, &body, &info),
        info,
    };
    // `TakeDamage` when there is an amount and the hit was not blocked; else the victim's
    // notify slot 28 (a hit without damage).
    let result = if amount != 0.0 && !c.no_react {
        cx.deliver_hit(&record)
    } else {
        cx.host.targets.touched(&record);
        Default::default()
    };
    // Step 21, on contact (owner notify slot 15, Attack state 0x141341B10; C 8.5.1).
    on_contact(cx, window, c.attacker, target.id, target.position, point, None);
    // On damage (causer notify slot 31 0x141442DE0; C 8.5.2): style points only when HP was
    // lost; no focus (`MyFocusPointAddHit` is 0).
    if result.damage_dealt > 0.0 {
        super::style::add_hit(cx, info.form_point);
    }
}

#[cfg(test)]
mod d3_tests {
    use super::*;

    #[test]
    fn unison_blizzard_end_grows_by_its_size_curve() {
        // TresProj_UnionBlizzardSize1: nothing until 0.4 s, 800 cm from 0.4333 s.
        let window = Window { key: (u16::MAX, u16::MAX), group: "UNISON_BLIZZARD_END", row: 0, row_name: "UNISON_BLIZZARD_END", hit_se: None, hit_se_asset: None, hit_effect: None, hit_effect_offset: 0.0, interval: 0.4, timer: 0.0, time: 0.0, initialised: true, previous: [Vec3::ZERO; 2], frozen: true, info: AttackCtx::default(), hit_list: Vec::new(), hits: 0, no_clash: true, clashed: false, map_hit: true, clash_list: [TargetId::NONE; 4] };
        // (D6b) The curve's keys from the sheet (sora-combat-shots UNISON_BLIZZARD#1), not a line.
        let row = super::super::tables::SHOT_SHAPES.iter().find(|r| r.id == "UNISON_BLIZZARD#1").expect("the row");
        assert_eq!(row.size_curve.len(), 6);
        let mut s = Shot { name: "UNISON_BLIZZARD_END", position: Vec3::ZERO, yaw: 0.0, life: 0.8, size: (150.0, 2000.0, 800.0), curve: row.size_curve, window, offset: Vec3::ZERO, convex: None, attach: None };
        for (t, r) in [(0.0, 0.0), (0.2, 0.0), (0.4, 0.0), (0.4167, 400.0), (0.4333, 800.0), (0.7, 800.0)] {
            s.window.time = t;
            assert!((s.radius() - r).abs() < 1.0, "{t}: {}", s.radius());
        }
    }
}

#[cfg(test)]
mod d6_tests {
    use super::*;

    /// TresProj_UnisonThunder's CONVEX: tres_colltest01 x (2,2,12) +(40,40,0)/s max (8,8,12).
    fn thunder() -> Convex {
        Convex { scale: Vec3::new(2.0, 2.0, 12.0), inc_scale: Vec3::new(40.0, 40.0, 0.0), max_scale: Vec3::new(8.0, 8.0, 12.0), yaw: 0.0 }
    }

    #[test]
    fn unison_thunders_column_grows_to_800_cm_in_0_15_s_and_is_1200_tall() {
        let c = thunder();
        for (t, r) in [(0.0, 200.0), (0.05, 400.0), (0.15, 800.0), (0.5, 800.0)] {
            let p = c.prism(Vec3::ZERO, t);
            assert!((p.radius.x - r).abs() < 0.01 && (p.radius.y - r).abs() < 0.01, "{t}: {:?}", p.radius);
            assert_eq!(p.half_height, 600.0);
        }
    }

    #[test]
    fn the_prism_is_a_20_gon_with_its_vertices_at_the_radius() {
        let p = thunder().prism(Vec3::new(0.0, 0.0, 250.0), 0.2);
        // A vertex direction (0 deg) reaches 800, the middle of an edge (9 deg) 800 x cos 9 deg.
        assert!(p.distance(Vec3::new(799.0, 0.0, 250.0)) == 0.0);
        let mid = 800.0 * 9f32.to_radians().cos();
        let d = Vec3::new(9f32.to_radians().cos(), 9f32.to_radians().sin(), 0.0);
        assert!(p.distance(d * (mid - 1.0) + Vec3::Z * 250.0) == 0.0);
        assert!((p.distance(d * (mid + 10.0) + Vec3::Z * 250.0) - 10.0).abs() < 0.05);
        // Top +850, bottom -350 (centre +250, half height 600).
        assert!(p.distance(Vec3::new(0.0, 0.0, 849.0)) == 0.0 && (p.distance(Vec3::new(0.0, 0.0, 860.0)) - 10.0).abs() < 0.01);
        assert!((p.distance(Vec3::new(0.0, 0.0, -360.0)) - 10.0).abs() < 0.01);
    }

    #[test]
    fn a_standing_body_beside_or_far_out_is_touched_by_the_column_and_not_past_it() {
        let p = thunder().prism(Vec3::new(0.0, 0.0, 250.0), 0.2);
        let body = |x: f32, y: f32| prism_contact(&p, Vec3::new(x, y, 40.0), Vec3::new(x, y, 140.0), 40.0);
        let (point, normal) = body(0.0, -100.0).expect("beside him");
        assert!(normal.y > 0.9 && (point.y - -60.0).abs() < 0.1, "{point:?} {normal:?}");
        assert!(body(-250.0, 0.0).is_some() && body(350.0, 350.0).is_some());
        assert!(body(830.0, 0.0).is_some(), "800 + the body's 40");
        assert!(body(850.0, 0.0).is_none());
        // A body far above the column's top is not touched.
        assert!(prism_contact(&p, Vec3::new(0.0, 0.0, 900.0), Vec3::new(0.0, 0.0, 1000.0), 40.0).is_none());
        // The capsule touches with its end: the nearest point of the segment is found.
        let p0 = thunder().prism(Vec3::ZERO, 0.0);
        assert!(prism_contact(&p0, Vec3::new(0.0, 0.0, 620.0), Vec3::new(0.0, 0.0, 2000.0), 40.0).is_some());
        assert!(prism_contact(&p0, Vec3::new(0.0, 0.0, 650.0), Vec3::new(0.0, 0.0, 2000.0), 40.0).is_none());
    }
}
