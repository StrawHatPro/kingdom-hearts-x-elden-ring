//! The environment-query runtime (`UEnvQueryManager` / `FEnvQueryInstance`) over the packed
//! queries (`build/sora_pack/party/ai/eqs/*.json`).
//!
//! Evidence: score normalisation and the eight scoring equations are KH3's own
//! (`EnvQueryTest` normalise 0x143C63FC0 [C]: Linear, Square, InverseLinear, SquareRoot,
//! InverseSquare, Sine `sin(n*pi)`, Sigmoid `1/(1+exp(6-12n))`, Constant; reference value = the
//! clamp minimum unless `bDefineReferenceValue`; all-equal scores add the factor only when they
//! equal 1). Filtering, options (first option with items wins) and SingleResult picking are stock
//! UE 4.17 [U]. The generators and Tres tests the friends use are transcribed from their
//! decompiled C (C3 / C4: addresses at each one); the contexts named in `context` are [D] unless
//! marked (the node table `sheets/port/party-ai-nodes.json` says which).
//! Queries run synchronously. The manager tick (0x143C6C920 [C4]: 10 ms budget,
//! `bTestQueriesUsingBreadth` one step per query round-robin, finished queries' delegates run in
//! the tick) finishes the friends' small queries in the frame they start; the BT tasks and
//! services deliver the result one tick later, as the game's callback reaching the tree does.

use crate::api::{ActorRef, ActorView, GimmickObstacle};
use crate::bb::{BbValue, Blackboard};
use crate::bt::Ctx;
use crate::nodes::validate::{norm, rotation};
use crate::pack::{enum_value, pb, pf, pnative, pobj, ps, AuxDef, EqsDef, Props};
use glam::{Vec2, Vec3};

/// One query item: a location, optionally an actor (or a lock-on component of it).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Item {
    pub pos: Vec3,
    pub actor: ActorRef,
    /// Lock-on component index (0 = the actor itself).
    pub comp: u8,
}

/// Extra named parameters (`QueryConfig` AIDynamicParam: Radius, AdjustValue ...).
pub type Params = Vec<(String, f32)>;

#[derive(Clone, Debug, Default)]
pub struct QueryResult {
    pub items: Vec<(Item, f32)>,
    /// Blackboard writes the query made while it ran (the SideMove generator's
    /// `SideMoveDirction` flip, 0x141225520); `apply_writes` puts them in.
    pub bb_writes: Vec<(String, BbValue)>,
}

impl QueryResult {
    pub fn best(&self) -> Option<Item> {
        self.items.first().map(|(i, _)| *i)
    }
    /// The game writes these during the query (the manager tick of the same frame).
    pub fn apply_writes(&self, bb: &mut Blackboard) {
        for (k, v) in &self.bb_writes {
            bb.set_by_name(k, v.clone());
        }
    }
}

fn yaw_dir(yaw_deg: f32) -> Vec3 {
    let r = yaw_deg.to_radians();
    Vec3::new(r.cos(), r.sin(), 0.0)
}

fn rot_dir(pitch: f32, yaw: f32) -> Vec3 {
    let (p, y) = (pitch.to_radians(), yaw.to_radians());
    Vec3::new(p.cos() * y.cos(), p.cos() * y.sin(), p.sin())
}

/// `FRotator(0, yaw, 0).RotateVector(v)` (UE FRotationMatrix rows, degrees).
fn rotate_yaw(v: Vec3, yaw: f32) -> Vec3 {
    let (s, c) = yaw.to_radians().sin_cos();
    Vec3::new(v.x * c - v.y * s, v.x * s + v.y * c, v.z)
}

/// `FRotator(0, 0, roll).RotateVector(v)`: M[1] = (0, cos, -sin), M[2] = (0, sin, cos).
fn rotate_roll(v: Vec3, roll: f32) -> Vec3 {
    let (s, c) = roll.to_radians().sin_cos();
    Vec3::new(v.x, v.y * c + v.z * s, -v.y * s + v.z * c)
}

/// A context's answer: points (with their actors) and a direction (rotation contexts).
#[derive(Clone, Debug, Default)]
struct CtxVal {
    items: Vec<Item>,
    radius: Vec<f32>,
    rot: Option<Vec3>,
    /// The context provides actors (`PrepareContext` for actors succeeds: Querier, Player,
    /// Target, Allies, Enemies), not only points / components. The OnRing generators take
    /// the actors' collision radii only then (0x1404C9920 / 0x1404C5150 [C4]).
    actors: bool,
}

fn actor_item(a: &ActorView) -> Item {
    Item { pos: a.position, actor: a.actor, comp: 0 }
}

/// Evaluate a context class by name [D unless noted].
fn context(c: &Ctx, name: &str, q: &Querier) -> CtxVal {
    let w = c.world;
    let p = w.player();
    let mut v = CtxVal::default();
    let push = |v: &mut CtxVal, a: &ActorView| {
        v.items.push(actor_item(a));
        v.radius.push(a.radius);
    };
    match name {
        "EnvQueryContext_Querier" | "TresEnvQueryContext_Querier" => {
            v.items.push(Item { pos: q.pos, actor: q.actor, comp: 0 });
            v.radius.push(q.radius);
            v.rot = Some(yaw_dir(q.yaw));
            v.actors = true;
        }
        "TresEnvQueryContext_QuerierFloorPosition" | "TresEnvQueryContext_QuerierFloorPositionFromNavmeshAndCollisionCheck" | "TresEnvQueryContext_QuerierFloorPositionFromNavmesh" => {
            let f = w.nav().project(q.pos, 100.0, 1000.0).unwrap_or(q.pos);
            v.items.push(Item { pos: f, actor: ActorRef::None, comp: 0 });
            v.radius.push(q.radius);
        }
        "TresEnvQueryContext_QuerierRotation" | "TresEnvQueryContext_QuerierRotationZ" => v.rot = Some(yaw_dir(q.yaw)),
        "TresEnvQueryContext_Player" | "TresEnvQueryContext_PlayerOrRidePawnForAI" => {
            push(&mut v, &p.actor);
            v.rot = Some(yaw_dir(p.actor.yaw));
            v.actors = true;
        }
        // 0x1404D6770 [C4]: the floor (collision trace, 5000 cm down) under the player = the
        // host's floor_position
        "TresEnvQueryContext_PlayerFloorPosition" | "TresEnvQueryContext_PlayerFloorPositionFromNavmeshAndCollisionCheck" | "TresEnvQueryContext_PlayerMercunaVolume" => {
            v.items.push(Item { pos: p.floor_position, actor: ActorRef::Player, comp: 0 });
            v.radius.push(p.actor.radius);
        }
        "TresEnvQueryContext_PlayerRotation" | "TresEnvQueryContext_PlayerRotationZ" => v.rot = Some(yaw_dir(p.actor.yaw)),
        "TresEnvQueryContext_PlayerCamera" => {
            let cam = w.camera();
            v.items.push(Item { pos: cam.position, actor: ActorRef::None, comp: 0 });
            v.radius.push(0.0);
            v.rot = Some(rot_dir(cam.pitch, cam.yaw));
        }
        // Target 0x1404DA2C0 [C4]: the querier's blackboard TargetActor as an actor
        "TresEnvQueryContext_Target" | "TresEnvQueryContext_TargetFloorPosition" | "TresEnvQueryContext_TargetFloorPositionFromNavmesh" | "TresEnvQueryContext_TargetFloorPositionFromNavmeshAndCollisionCheck" | "TresEnvQueryContext_TargetCenterPosition" => {
            let t = c.bb.key_id("TargetActor").map(|k| c.bb.actor(k)).unwrap_or_default();
            if let Some(a) = w.actor(t) {
                let pos = if name.ends_with("CenterPosition") { a.center } else { a.position };
                v.items.push(Item { pos, actor: a.actor, comp: 0 });
                v.radius.push(a.radius);
                v.rot = Some(yaw_dir(a.yaw));
                v.actors = name == "TresEnvQueryContext_Target";
            }
        }
        // Allies 0x1404D3BF0 [C4]: the AI manager's actors of the querier's own team
        // (FUN_140439810 relation 0) without the querier (ride pawns substituted)
        "TresEnvQueryContext_Allies" => {
            let mut al = Vec::new();
            w.allies(&mut al);
            for a in al.iter().filter(|a| a.actor != q.actor) {
                push(&mut v, a);
            }
            // the player is on the friends' team
            push(&mut v, &p.actor);
            v.actors = true;
        }
        // (FOL2) the other members' random-walk spots (crate::formation records them): KH3's walk-round spots keep 200 cm
        // from each other; the port had no such context and two friends could walk to one spot
        "TresEnvQueryContext_AllyActionDestinations" => {
            if crate::formation::ON.load(std::sync::atomic::Ordering::Relaxed) {
                let now = c.st.time;
                for (m, (d, at)) in c.shared.action_dest.lock().unwrap_or_else(|e| e.into_inner()).iter() {
                    if ActorRef::Member(*m) != q.actor && *m != q.member && (now - at).abs() < crate::formation::ACTION_DEST_LIFE {
                        v.items.push(Item { pos: *d, actor: ActorRef::Member(*m), comp: 0 });
                        v.radius.push(0.0);
                    }
                }
            }
        }
        "TresEnvQueryContext_AllyDestinations" => {
            for (m, d) in &c.shared.destinations {
                if ActorRef::Member(*m) != q.actor {
                    v.items.push(Item { pos: *d, actor: ActorRef::Member(*m), comp: 0 });
                    v.radius.push(0.0);
                }
            }
        }
        // Enemies 0x1404D4CF0 [C4]: the AI manager's actors of the hostile teams (relation 2)
        "TresEnvQueryContext_Enemies" => {
            let mut en = Vec::new();
            w.enemies(&mut en);
            for a in en.iter().filter(|a| a.alive && a.hostile) {
                push(&mut v, a);
            }
            v.actors = true;
        }
        "TresNpcEnvQueryContext_LockOnTarget" | "TresNpcEnvQueryContext_LockOnTargetAll" | "TresEnvQueryContext_PlayerLockOnTargetComponent" => {
            // lock-on target components of the hostile, lockable enemies (the player's lock-on
            // target only for the Player... context)
            let mut en = Vec::new();
            w.enemies(&mut en);
            for a in en.iter().filter(|a| a.alive && a.hostile && a.lockable) {
                if name == "TresEnvQueryContext_PlayerLockOnTargetComponent" && a.actor != p.lock_on {
                    continue;
                }
                v.items.push(Item { pos: a.center, actor: a.actor, comp: 1 });
                v.radius.push(a.radius);
            }
        }
        "TresEnvQueryContext_FriendHomePosition" | "TresEnvQueryContext_FriendHomePositionSide" => {
            let home = home_position(c, q.member, name.ends_with("Side"));
            v.items.push(Item { pos: home, actor: ActorRef::None, comp: 0 });
            v.radius.push(0.0);
        }
        "TresEnvQueryContext_NullLocation" => v.rot = Some(Vec3::X),
        // [D] contexts named after what they answer
        "TresEnvQueryContext_PlayerFloorFollowPosition" => {
            v.items.push(Item { pos: p.floor_position, actor: ActorRef::Player, comp: 0 });
            v.radius.push(p.actor.radius);
        }
        "TresEnvQueryContext_PlayerCenterPositionInMercunaVolume" => {
            v.items.push(Item { pos: p.actor.center, actor: ActorRef::Player, comp: 0 });
            v.radius.push(p.actor.radius);
        }
        "TresEnvQueryContext_QuerierCenterPosition" => {
            v.items.push(Item { pos: q.pos + Vec3::Z * q.half_height, actor: q.actor, comp: 0 });
            v.radius.push(q.radius);
        }
        "TresEnvQueryContext_QuerierFloorOrWaterSurfacePosition" => {
            v.items.push(Item { pos: w.nav().project(q.pos, 100.0, 1000.0).unwrap_or(q.pos), actor: ActorRef::None, comp: 0 });
            v.radius.push(q.radius);
        }
        "TresEnvQueryContext_Destination" | "TresEnvQueryContext_DestinationForPrecede" => {
            if let Some(d) = c.bb.key_id("Destination").map(|k| c.bb.vector(k)).filter(|d| crate::bb::is_valid_location(*d)) {
                v.items.push(Item { pos: d, actor: ActorRef::None, comp: 0 });
                v.radius.push(0.0);
            }
        }
        // (party AIX, read 2026-10-15) PlayerFollowingSide 0x1404D6C80 / _Destination 0x1404D7150 [C]: the player
        // movement's average direction (+0xC4C, MotionTrack) turned by FRotator(0, 90, 0) (FUN_142d8d890) plus the
        // player's right axis (FRotationMatrix row Y, FUN_1404bfa00), normalised (zero below 1e-8), negated when it
        // points away from the querier (from the blackboard Destination for _Destination; an invalid Destination
        // gives no rotation) - 2D sign test against (side point - player). Before [D]: toward the friend's fixed
        // home slot (+-70 deg of the facing), which pulled a friend across behind a running leader.
        "TresEnvQueryContext_PlayerFollowingSide" | "TresEnvQueryContext_PlayerFollowingSide_Destination" => {
            let side = if name.ends_with("_Destination") { c.bb.key_id("Destination").map(|k| c.bb.vector(k)).filter(|d| crate::bb::is_valid_location(*d)) } else { Some(q.pos) };
            if let Some(sp) = side {
                let a = c.st.player_motion.avg_dir;
                let dir = Vec3::new(-a.y, a.x, a.z) + yaw_dir(p.actor.yaw + 90.0);
                let l2 = dir.length_squared();
                let mut dir = if l2 < 1e-8 { Vec3::ZERO } else { dir / l2.sqrt() };
                let to = sp - p.actor.position;
                if dir.x * to.x + dir.y * to.y < 0.0 {
                    dir = -dir;
                }
                v.rot = Some(dir);
            }
        }
        _ => {}
    }
    v
}

/// The friend's home position next to the player (`FriendHomePosDataTable` row = party slot:
/// base length / angle from the player's facing) [A]; which of base / middle / large applies is
/// [?] (base used).
pub fn home_position(c: &Ctx, member: u8, _side: bool) -> Vec3 {
    let p = c.world.player();
    let row = c.pack.tables.get("FriendHomePosDataTable").and_then(|t| t.rows.get(member as usize));
    let (len, rot) = row.map(|(_, r)| (pf(r, "m_BaseLen", 180.0), pf(r, "m_BaseRot", 70.0))).unwrap_or((180.0, 70.0));
    p.floor_position + yaw_dir(p.actor.yaw + rot) * len
}

#[derive(Clone, Copy, Debug)]
pub struct Querier {
    /// The pawn's origin (foot level in the port; UE's actor location is the capsule centre
    /// = `pos + Z * half_height`).
    pub pos: Vec3,
    pub yaw: f32,
    pub radius: f32,
    pub half_height: f32,
    pub actor: ActorRef,
    pub member: u8,
}

fn param(p: &Props, k: &str, params: &Params, default: f32) -> f32 {
    // AIDataProviderFloatValue: a bound query parameter overrides DefaultValue
    if let Some(o) = pobj(p, k) {
        if let Some(b) = o.get("DataBinding").and_then(|b| b.as_object()) {
            if let Some(pp) = b.get("p").and_then(|x| x.as_object()) {
                if let Some(name) = pp.get("ParamName").and_then(|x| x.as_str()) {
                    if let Some((_, v)) = params.iter().find(|(n, _)| n == name) {
                        return *v;
                    }
                }
            }
        }
    }
    pf(p, k, default)
}

/// AIDataProviderBoolValue: a bound parameter (!= 0) overrides DefaultValue.
fn param_bool(p: &Props, k: &str, params: &Params, default: bool) -> bool {
    if let Some(o) = pobj(p, k) {
        if let Some(name) = o.get("DataBinding").and_then(|b| b.get("p")).and_then(|pp| pp.get("ParamName")).and_then(|x| x.as_str()) {
            if let Some((_, v)) = params.iter().find(|(n, _)| n == name) {
                return *v != 0.0;
            }
        }
    }
    pb(p, k, default)
}

/// A generator's `ProjectionData` (`FEnvTraceData`) [C4]: `UEnvQueryGenerator_ProjectedPoints`
/// ctor 0x143C72310 (ProjectDown / ProjectUp 1024, ExtentX 0, TraceMode Navigation); the Tres
/// generators' ctors set TraceMode None (Context 0x1404C0E30, OnRing 0x1404C0FD0, OnRing3D
/// 0x1404C0EA0), SideMove 0x14121C730 keeps Navigation.
struct Trace {
    /// 0 None, 1 Navigation, 2 Geometry.
    mode: u8,
    up: f32,
    down: f32,
    post: f32,
}

fn trace_of(class: &str, p: &Props) -> Trace {
    let default_mode = match class {
        "TresEnvQueryGenerator_Context" | "TresEnvQueryGenerator_OnRing" | "TresEnvQueryGenerator_OnRing3D" | "EnvQueryGenerator_CurrentLocation" => 0,
        _ => 1,
    };
    let d = pobj(p, "ProjectionData");
    let mode = d
        .and_then(|d| d.get("TraceMode"))
        .and_then(|v| v.as_str())
        .map(|s| match enum_value(s) {
            "Navigation" => 1,
            "Geometry" => 2,
            _ => 0,
        })
        .unwrap_or(default_mode);
    let f = |k: &str, def: f32| d.and_then(|d| d.get(k)).and_then(|v| v.as_f64()).map(|v| v as f32).unwrap_or(def);
    Trace { mode, up: f("ProjectUp", 1024.0), down: f("ProjectDown", 1024.0), post: f("PostProjectionVerticalOffset", 0.0) }
}

/// `ProjectAndFilterNavPoints` (vt+600 0x143C882C0, stock) [C4]: Navigation =
/// `RunNavProjection` 0x143C6A230 in Discard mode (the point moved by (Up - Down) / 2 with the
/// extent (Up + Down) / 2, i.e. the floor within [z - Down, z + Up]; points without one are
/// dropped; + PostProjectionVerticalOffset); Geometry = the physics projection (the port's
/// `Nav::project` stands in for both); None keeps the points.
fn project_points(c: &Ctx, t: &Trace, pts: Vec<Item>, out: &mut Vec<Item>) {
    for it in pts {
        if t.mode == 0 {
            out.push(it);
        } else if let Some(f) = c.world.nav().project(it.pos, t.up, t.down) {
            out.push(Item { pos: f + Vec3::Z * t.post, ..it });
        }
    }
}

/// `CeilToInt` as UE builds it (`-(cvt(-0.5 - 2x) >> 1)`): the ceiling for finite values.
fn ceil_to_int(x: f32) -> i32 {
    if !x.is_finite() {
        return 0;
    }
    x.ceil() as i32
}

/// The OnRing ring helper 0x1404C90C0 [C3]: CeilToInt(arc length / spacing) points, one more
/// for a partial arc; from -Angle / 2 in steps of Angle / (count - partial), about `base_yaw`.
fn ring(centre: Vec3, base_yaw: f32, arc: f32, radius: f32, spacing: f32, out: &mut Vec<Item>) {
    let partial = (arc / 360.0 < 1.0) as i32;
    let count = partial + ceil_to_int(radius * std::f32::consts::TAU * arc / 360.0 / spacing);
    let div = (count - partial).max(1);
    for j in 0..count.max(0) {
        let a = -arc * 0.5 + j as f32 * (arc / div as f32);
        out.push(Item { pos: centre + yaw_dir(base_yaw + a) * radius, actor: ActorRef::None, comp: 0 });
    }
}

fn generate(c: &Ctx, g: &AuxDef, q: &Querier, params: &Params, out: &mut Vec<Item>, writes: &mut Vec<(String, BbValue)>) {
    let p = &g.p;
    let nav = c.world.nav();
    let trace = trace_of(&g.class, p);
    match g.class.as_str() {
        // OnRing GenerateItems 0x1404C6420 (rings 0x1404C90C0, centres 0x1404C9920, ctor
        // 0x1404C0FD0) [C3] and OnRing3D 0x1404C5150 (ctor 0x1404C0EA0) [C4]: the centres are the
        // context's actors with their collision radius (FUN_140540e20), else its points (radius
        // 0); MinRadius 800 (>= 0), MaxRadius 1000 (>= MinRadius), MinRadiusItemSpacing 100
        // (>= 1), MaxRadiusItemSpacing 100 x (bDefineMaxRadiusItemSpacing ? Ratio : 1) (>= 1),
        // Angle 360 clamped to [0, 360]; radii: CeilToInt((Max - Min) / min(spacings)) + 1 evenly
        // from Min to Max, the spacing lerps Min -> Max; ring radius = radius + the context
        // actor's (all modes but ContextCenter) + the querier's (QuerierAndContextRadius).
        // OnRing3D: ItemSpacingZ 100, MinHeight 0, MaxHeight 500 (not clamped); heights
        // CeilToInt((Max - Min) / SpacingZ) + 1 from MinHeight in SpacingZ steps (centre z +
        // MinHeight + k SpacingZ); then ProjectAndFilterNavPoints.
        "TresEnvQueryGenerator_OnRing" | "TresEnvQueryGenerator_OnRing3D" => {
            let centre_ctx = pnative(p, "CircleCenter").unwrap_or_else(|| "EnvQueryContext_Querier".into());
            let cv = context(c, &centre_ctx, q);
            let min_r = param(p, "MinRadius", params, 800.0).max(0.0);
            let max_r = param(p, "MaxRadius", params, 1000.0).max(min_r);
            let sp_min = param(p, "MinRadiusItemSpacing", params, 100.0).max(1.0);
            let ratio = if pb(p, "bDefineMaxRadiusItemSpacing", false) { pf(p, "MaxRadiusItemSpacingRatio", 1.0) } else { 1.0 };
            let sp_max = (param(p, "MaxRadiusItemSpacing", params, 100.0) * ratio).max(1.0);
            let arc = param(p, "Angle", params, 360.0).clamp(0.0, 360.0);
            let dir = pobj(p, "ArcDirection")
                .and_then(|d| d.get("Rotation").and_then(|r| r.as_str()).map(|r| r.rsplit('.').next().unwrap_or(r).to_string()))
                .map(|r| context(c, &r, q).rot.unwrap_or(Vec3::X))
                .unwrap_or(Vec3::X);
            let base_yaw = dir.y.atan2(dir.x).to_degrees();
            let heights: Vec<f32> = if g.class.ends_with("3D") {
                let sz = param(p, "ItemSpacingZ", params, 100.0);
                let lo = param(p, "MinHeight", params, 0.0);
                let hi = param(p, "MaxHeight", params, 500.0);
                let n = if sz > 0.0 { ceil_to_int((hi - lo) / sz) + 1 } else { 1 };
                (0..n.max(0)).map(|k| lo + k as f32 * sz).collect()
            } else {
                vec![0.0]
            };
            let mode = ps(p, "RadiusMode").map(enum_value).unwrap_or("ContextCenter");
            let span = max_r - min_r;
            let radii = ceil_to_int(span / sp_min.min(sp_max)) + 1;
            let ring_step = if radii >= 2 { span / (radii - 1) as f32 } else { 0.0 };
            let querier_r = if mode == "QuerierAndContextRadius" { q.radius } else { 0.0 };
            let mut pts = Vec::new();
            for (ci, centre) in cv.items.iter().enumerate() {
                let ctx_r = if mode == "ContextCenter" || !cv.actors { 0.0 } else { cv.radius.get(ci).copied().unwrap_or(0.0) };
                for h in &heights {
                    let cpos = centre.pos + Vec3::Z * *h;
                    for k in 0..radii.max(0) {
                        let r = min_r + ring_step * k as f32;
                        let t = if min_r != max_r { (r - min_r) / span } else { 0.0 };
                        let spacing = (1.0 - t) * sp_min + t * sp_max;
                        ring(cpos, base_yaw, arc, ctx_r + querier_r + r, spacing, &mut pts);
                    }
                }
            }
            project_points(c, &trace, pts, out);
        }
        // GenerateItems 0x1404C4BB0 [C4] (ctor 0x1404C0E30: Context Querier, ItemType =
        // ContextItemType = Point, TraceMode None; PostLoad 0x1404D2D30 ItemType =
        // ContextItemType): ActionDefinition items -> the context's action definitions
        // (FUN_1404bf3f0; none in the port), Actor -> the context's actors, Point -> the
        // context's points + ProjectAndFilterNavPoints, SceneComponent -> its components.
        "TresEnvQueryGenerator_Context" => {
            let ctx_name = pnative(p, "Context").unwrap_or_else(|| "EnvQueryContext_Querier".into());
            let ty = pnative(p, "ContextItemType").or_else(|| pnative(p, "ItemType")).unwrap_or_else(|| "EnvQueryItemType_Point".into());
            let cv = context(c, &ctx_name, q);
            if ty.contains("ActionDefinition") {
                // no action-definition contexts in the friends' queries
            } else if ty.ends_with("ItemType_Actor") || ty.ends_with("ItemType_ActorBase") {
                if cv.actors {
                    out.extend(cv.items.iter().copied());
                }
            } else if ty.ends_with("SceneComponent") {
                out.extend(cv.items.iter().copied());
            } else {
                let pts = cv.items.iter().map(|i| Item { pos: i.pos, actor: ActorRef::None, comp: 0 }).collect();
                project_points(c, &trace, pts, out);
            }
        }
        "EnvQueryGenerator_CurrentLocation" => out.push(Item { pos: q.pos, actor: ActorRef::None, comp: 0 }),
        "EnvQueryGenerator_SimpleGrid" | "EnvQueryGenerator_PathingGrid" | "EnvQueryGenerator_Mercuna_Sphere" => {
            let half = param(p, "GridSize", params, 500.0);
            let step = param(p, "SpaceBetween", params, 100.0).max(10.0);
            let n = (half / step).floor() as i32;
            let mut pts = Vec::new();
            for i in -n..=n {
                for j in -n..=n {
                    let pos = q.pos + Vec3::new(i as f32 * step, j as f32 * step, 0.0);
                    if g.class == "EnvQueryGenerator_PathingGrid" && !nav.path_exists(q.pos, pos) {
                        continue;
                    }
                    pts.push(Item { pos, actor: ActorRef::None, comp: 0 });
                }
            }
            project_points(c, &trace, pts, out);
        }
        "TresNpcEnvQueryGenerator_SideMove" => side_move(c, p, q, params, &trace, out, writes),
        "EnvQueryGenerator_Composite" => {
            if let Some(gens) = p.get("Generators").and_then(|v| v.as_array()) {
                for g2 in gens {
                    if let Some(o) = g2.as_object() {
                        let cls = o.get("$class").and_then(|x| x.as_str()).unwrap_or("").to_string();
                        let pp = o.get("p").and_then(|x| x.as_object()).cloned().unwrap_or_default();
                        generate(c, &AuxDef { class: cls, name: String::new(), p: pp }, q, params, out, writes);
                    }
                }
            }
        }
        _ => {}
    }
}

/// `TresNpcEnvQueryGenerator_SideMove` GenerateItems 0x141225520 [C4] (ctor 0x14121C730: Radius
/// 300, AdjustValue 100, ForwardPowerRate 1, TraceMode Navigation):
/// - the target point = the blackboard's LockOnTargetComponent location (a lock-on component),
///   else TargetActor's; the reach = the querier's collision radius + TargetActor's; the side =
///   the blackboard float `SideMoveDirction`;
/// - no target point (the zero vector) -> one item, the querier's location;
/// - else item 0 = the querier's location, and with d = target - querier (2D) and rot(d) its
///   yaw: when the side point rot(d)(0, side x 100, 0) is >= 150 deg off the querier's facing
///   the side flips (and is written back to `SideMoveDirction`); t = clamp((|d| - (Radius +
///   reach)) / AdjustValue (<= 0 -> 1), -1, 1); v = normalise(t x ForwardPowerRate (>= 0),
///   side or 1, 0); while swimming (FUN_140546020) v is rolled by -(side x pitch(target -
///   querier)); then three items at querier + rot(d)(v x s), s = radius + 70, +100, +100;
/// - ProjectAndFilterNavPoints.
fn side_move(c: &Ctx, p: &Props, q: &Querier, params: &Params, trace: &Trace, out: &mut Vec<Item>, writes: &mut Vec<(String, BbValue)>) {
    let view = |a: ActorRef| crate::nodes::view(c, a);
    let comp = c.bb.key_id("LockOnTargetComponent").map(|k| c.bb.object(k)).unwrap_or_default();
    let tgt = c.bb.key_id("TargetActor").map(|k| c.bb.actor(k)).and_then(view);
    let comp_view = match comp {
        crate::bb::BbObject::Component(a, _) => view(a),
        _ => None,
    };
    let target = comp_view.map(|v| v.center).or(tgt.map(|t| t.center)).unwrap_or(Vec3::ZERO);
    let reach = q.radius + tgt.map(|t| t.radius).unwrap_or(0.0);
    let side_key = c.bb.key_id("SideMoveDirction");
    let mut side = side_key.map(|k| c.bb.float(k)).unwrap_or(0.0);
    let mut pts = vec![Item { pos: q.pos, actor: ActorRef::None, comp: 0 }];
    if target != Vec3::ZERO {
        let root = q.pos + Vec3::Z * q.half_height;
        let d = (target - root).with_z(0.0);
        let dyaw = rotation(d).1;
        let radius = param(p, "Radius", params, 300.0);
        let mut adjust = param(p, "AdjustValue", params, 100.0).max(0.0);
        if adjust <= 0.0 {
            adjust = 1.0;
        }
        let side_vec = rotate_yaw(Vec3::new(0.0, side * 100.0, 0.0), dyaw);
        let side_yaw = if side_vec.length_squared() > 0.0 { rotation(side_vec).1 } else { 0.0 };
        if norm(side_yaw - q.yaw).abs() >= 150.0 {
            side = -side;
            if side_key.is_some() {
                writes.push(("SideMoveDirction".into(), BbValue::Float(side)));
            }
        }
        let fwd_rate = param(p, "ForwardPowerRate", params, 1.0).max(0.0);
        let t = ((d.length() - (radius + reach)) / adjust).clamp(-1.0, 1.0);
        let fwd = t * fwd_rate;
        let mut v = Vec3::new(fwd, if side != 0.0 { side } else { 1.0 }, 0.0);
        if v.length_squared() > 1e-8 {
            v = v.normalize();
        }
        let swimming = c.body.function("IsSwimming").map(|x| x != 0.0).unwrap_or(c.body.view().swimming);
        if swimming {
            let pitch = rotation(target - root).0;
            v = rotate_roll(v, -(side * pitch));
        }
        let mut s = 0.0;
        for k in 1..4 {
            s += if k == 1 { q.radius + 70.0 } else { 100.0 };
            pts.push(Item { pos: q.pos + rotate_yaw(v * s, dyaw), actor: ActorRef::None, comp: 0 });
        }
    }
    project_points(c, trace, pts, out);
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Purpose {
    Filter,
    Score,
    Both,
}

/// One test's outcome for an item: no value (the test did not touch it: it passes and scores 0),
/// a value, or forced failure (`ForceItemState(Failed)`: the item is dropped).
#[derive(Clone, Copy, Debug, PartialEq)]
enum TV {
    Skip,
    Val(f32),
    Fail,
}

/// The items' bounding box over the live ones.
fn items_box(items: &[Item], alive: &[bool]) -> (Vec3, Vec3) {
    let mut mn = Vec3::splat(3.4e38);
    let mut mx = Vec3::splat(-3.4e38);
    for (i, it) in items.iter().enumerate() {
        if alive[i] {
            mn = mn.min(it.pos);
            mx = mx.max(it.pos);
        }
    }
    (mn, mx)
}

/// A whole test run (the tests that look at all items or draw once), else per item.
fn test_values(c: &Ctx, t: &AuxDef, q: &Querier, items: &[Item], alive: &[bool], params: &Params) -> Vec<TV> {
    let p = &t.p;
    let w = c.world;
    let n = items.len();
    match t.class.as_str() {
        // RunTest 0x1404DCB50 [C4] (ctor 0x1404C1330: Context Querier, bAddQuerierRadius on,
        // bUseGimmickObstacle off, bool test): the obstacles are the context's actors' bounds
        // (bUseGimmickObstacle off: centre, radius max(extent x, y), half height extent z) or
        // their UTresGimmickObstacleComponents (on); an obstacle outside the items' box widened
        // by its own size (the actors' case widens by the querier's bounds instead) is dropped;
        // the query radius adds the querier's max(extent x, y) when bAddQuerierRadius; no
        // obstacle -> the test does not run; an item overlaps when dx² + dy² <= (r + rq)² and
        // |z - centre z| <= half height; the bool (BoolValue) test then filters / scores.
        "TresEnvQueryTest_ActorOverlap" => {
            let ctx_name = pnative(p, "Context").unwrap_or_else(|| "EnvQueryContext_Querier".into());
            let rq = if pb(p, "bAddQuerierRadius", true) { q.radius } else { 0.0 };
            let (mn, mx) = items_box(items, alive);
            let mut obs: Vec<(Vec2, f32, f32, f32)> = Vec::new();
            if pb(p, "bUseGimmickObstacle", false) {
                if ctx_name == "TresEnvQueryContext_GimmickObstacles" {
                    let mut list: Vec<GimmickObstacle> = Vec::new();
                    w.gimmick_obstacles(&mut list);
                    for o in list {
                        let (r, hh) = (o.radius, o.half_height);
                        let zc = o.base.z + hh;
                        if mx.x < o.base.x - r || o.base.x + r < mn.x || mx.y < o.base.y - r || o.base.y + r < mn.y || mx.z < zc - hh || zc + hh < mn.z {
                            continue;
                        }
                        obs.push((o.base.truncate(), zc, (r + rq) * (r + rq), hh));
                    }
                }
            } else {
                let cv = context(c, &ctx_name, q);
                if cv.actors {
                    let (qx, qz) = (q.radius, q.half_height);
                    for it in &cv.items {
                        let Some(a) = crate::nodes::view(c, it.actor) else { continue };
                        let o = a.center;
                        if mx.x < o.x - qx || o.x + qx < mn.x || mx.y < o.y - qx || o.y + qx < mn.y || mx.z < o.z - qz || o.z + qz < mn.z {
                            continue;
                        }
                        obs.push((o.truncate(), o.z, (a.radius + rq) * (a.radius + rq), a.half_height));
                    }
                }
            }
            if obs.is_empty() {
                return vec![TV::Skip; n];
            }
            items
                .iter()
                .map(|it| {
                    let hit = obs.iter().any(|(xy, z, r2, hz)| (it.pos.truncate() - *xy).length_squared() <= *r2 && (it.pos.z - z).abs() <= *hz);
                    TV::Val(hit as i32 as f32)
                })
                .collect()
        }
        // RunTest 0x1404E0F50 [C4] (ctor 0x1404C1750: PathStart / PathEnd Querier, float test):
        // a FindPathSync (FilterClass) for every start point x end point pair, the successful
        // paths kept; the value = the distance from the item to the closest point on them
        // (FUN_140530e20: closest point over the path's segments); no path -> the item is
        // skipped (ForceItemState passed, no score).
        "TresEnvQueryTest_DistanceToPath" => {
            let starts = context(c, &pnative(p, "PathStart").unwrap_or_else(|| "EnvQueryContext_Querier".into()), q).items;
            let ends = context(c, &pnative(p, "PathEnd").unwrap_or_else(|| "EnvQueryContext_Querier".into()), q).items;
            let mut paths: Vec<Vec<Vec3>> = Vec::new();
            for s in &starts {
                for e in &ends {
                    let mut pts = Vec::new();
                    if w.nav().path(s.pos, e.pos, &mut pts) && !pts.is_empty() {
                        paths.push(pts);
                    }
                }
            }
            items
                .iter()
                .map(|it| {
                    let mut best: Option<f32> = None;
                    for path in &paths {
                        let mut cp = path[0];
                        for seg in path.windows(2) {
                            let x = closest_on_segment(it.pos, seg[0], seg[1]);
                            if x.distance_squared(it.pos) < cp.distance_squared(it.pos) {
                                cp = x;
                            }
                        }
                        let d = cp.distance(it.pos);
                        if best.map(|b| d < b).unwrap_or(true) {
                            best = Some(d);
                        }
                    }
                    best.map(TV::Val).unwrap_or(TV::Skip)
                })
                .collect()
        }
        // RunTest 0x1404EC590 [C4] (ctor 0x1404C23C0: VolumeClass Volume, bSkipIfNoVolumeFound
        // on, bCheckTags off, bool test): the volumes of VolumeClass whose bounds overlap the
        // items' box (tags not checked: no tagged volumes in the port); none and
        // bSkipIfNoVolumeFound -> the test does not run; an item passes inside a volume
        // (strictly inside its bounds, then EncompassesPoint: a box here).
        "TresEnvQueryTest_Volume" => {
            let class = pnative(p, "VolumeClass").unwrap_or_else(|| "Volume".into());
            let (mn, mx) = items_box(items, alive);
            let mut boxes = Vec::new();
            w.volumes(&class, &mut boxes);
            boxes.retain(|(a, b)| a.x <= mx.x && mn.x <= b.x && a.y <= mx.y && mn.y <= b.y && a.z <= mx.z && mn.z <= b.z);
            if pb(p, "bSkipIfNoVolumeFound", true) && boxes.is_empty() {
                return vec![TV::Skip; n];
            }
            items
                .iter()
                .map(|it| {
                    let x = it.pos;
                    let inside = boxes.iter().any(|(a, b)| a.x < x.x && x.x < b.x && a.y < x.y && x.y < b.y && a.z < x.z && x.z < b.z);
                    TV::Val(inside as i32 as f32)
                })
                .collect()
        }
        // RunTest 0x1412324D0 [C4] (ctor 0x14121C840): the querier must be a char pawn; the
        // context's first actor T (its enemy parent with m_bUseEnemyParent: no multi-part
        // enemies in the port); desired = T's yaw - normalise(m_Angle); bearing = yaw(querier -
        // T, 2D); diff = normalise(desired - bearing); the reference yaw = desired when |diff| <
        // 45, else bearing +- m_WayAngle towards it; value = 180 - |normalise(yaw(item - T) -
        // reference)| (float test).
        "TresNpcEnvQueryTest_Around" => {
            let cv = context(c, &pnative(p, "m_Context").unwrap_or_else(|| "EnvQueryContext_Querier".into()), q);
            let Some(first) = cv.items.first().filter(|_| cv.actors) else { return vec![TV::Skip; n] };
            let Some(tv) = crate::nodes::view(c, first.actor) else { return vec![TV::Skip; n] };
            let angle = norm(param(p, "m_Angle", params, 0.0));
            let way = param(p, "m_WayAngle", params, 0.0);
            let _parent = param_bool(p, "m_bUseEnemyParent", params, false);
            let desired = tv.yaw - angle;
            let d = (q.pos - tv.position).with_z(0.0);
            let bearing = rotation(d).1;
            let diff = norm(desired - bearing);
            let reference = if diff.abs() >= 45.0 { rotation(rotate_yaw(d, if diff <= 0.0 { -way } else { way })).1 } else { desired };
            items.iter().map(|it| TV::Val(180.0 - norm(rotation(it.pos - tv.position).1 - reference).abs())).collect()
        }
        // RunTest 0x1412358F0 [C4] (ctor 0x14121CB50): the escape direction E from the
        // EscapeContext's actors plus, with m_bEscapeToPlayerTeam, the AI manager's player team
        // (FUN_14043cdb0 team 0: the player and both friends, the querier included): for each
        // actor whose 2D gap |querier - actor| - (its radius + the querier's) is <= m_Radius, E
        // += unit(querier - actor) x (m_Radius - gap); E /= that count. When no actor counts or E
        // is zero: with m_bEscapeToTargetActor, E = unit(querier - TargetActor) (else
        // SubTargetActor), 2D. E zero -> every item FAILS; else value = |normalise(yaw(item -
        // querier) - yaw(E))| (float test; scored InverseSquare by the friends).
        "TresNpcEnvQueryTest_Escape" => {
            let radius = param(p, "m_Radius", params, 0.0);
            let to_target = param_bool(p, "m_bEscapeToTargetActor", params, false);
            let to_team = param_bool(p, "m_bEscapeToPlayerTeam", params, false);
            let cv = context(c, &pnative(p, "EscapeContext").unwrap_or_else(|| "EnvQueryContext_Querier".into()), q);
            let mut others: Vec<(Vec3, f32)> = Vec::new();
            if cv.actors {
                for (i, it) in cv.items.iter().enumerate() {
                    others.push((it.pos, cv.radius.get(i).copied().unwrap_or(0.0)));
                }
            }
            if to_team {
                let pl = w.player().actor;
                others.push((pl.position, pl.radius));
                let mut al = Vec::new();
                w.allies(&mut al);
                for a in &al {
                    if a.actor == q.actor {
                        others.push((q.pos, q.radius));
                    } else {
                        others.push((a.position, a.radius));
                    }
                }
            }
            let unit = |v: Vec2| if v.length_squared() > 1e-8 { v.normalize() } else { v };
            let mut e = Vec2::ZERO;
            let mut count = 0;
            for (pos, r) in &others {
                let d = (q.pos - *pos).truncate();
                let gap = d.length() - (r + q.radius);
                if gap <= radius {
                    e += unit(d) * (radius - gap);
                    count += 1;
                }
            }
            if count >= 1 && e != Vec2::ZERO {
                e /= count as f32;
            } else if to_target {
                let t = ["TargetActor", "SubTargetActor"].iter().find_map(|k| c.bb.key_id(k).map(|id| c.bb.actor(id)).filter(|a| !a.is_none()).and_then(|a| crate::nodes::view(c, a)));
                if let Some(t) = t {
                    e = unit((q.pos - t.position).truncate());
                }
            }
            if e == Vec2::ZERO {
                return vec![TV::Fail; n];
            }
            let eyaw = e.y.atan2(e.x).to_degrees();
            items.iter().map(|it| TV::Val(norm(rotation(it.pos - q.pos).1 - eyaw).abs())).collect()
        }
        // RunTest 0x141233850 [C4]: runs only when the AI manager has NPC-mode free-walk points
        // (+0x7E8 count > 0): an item passes (value 1) when it is an unused way point of the
        // querier's uid, else fails. The port has none: the test does not run.
        "TresNpcEnvQueryTest_CanUseFreeMovePoint" => vec![TV::Skip; n],
        _ => items.iter().map(|it| test_value(c, t, q, it, params).map(TV::Val).unwrap_or(TV::Skip)).collect(),
    }
}

fn closest_on_segment(p: Vec3, a: Vec3, b: Vec3) -> Vec3 {
    let ab = b - a;
    let l2 = ab.length_squared();
    if l2 <= 0.0 {
        return a;
    }
    a + ab * ((p - a).dot(ab) / l2).clamp(0.0, 1.0)
}

/// One test's raw value for an item: Some(value) or None (no value: item keeps 0 score).
fn test_value(c: &Ctx, t: &AuxDef, q: &Querier, item: &Item, params: &Params) -> Option<f32> {
    let p = &t.p;
    let w = c.world;
    let ctx_points = |key: &str, default: &str| -> CtxVal { context(c, &pnative(p, key).unwrap_or_else(|| default.into()), q) };
    match t.class.as_str() {
        "EnvQueryTest_Distance" => {
            let cv = ctx_points("DistanceTo", "EnvQueryContext_Querier");
            let mode = ps(p, "TestMode").map(enum_value).unwrap_or("Distance3D");
            cv.items.iter().map(|o| dist_mode(mode, item.pos, o.pos)).reduce(f32::min)
        }
        // RunTest 0x1404E7E80 [C]: min over the context's actors (else its points) of the
        // distance minus (item actor radius + context actor radius) with DistanceToCollisionBounds
        // (FUN_140540e20; navigation bounds FUN_140540ed0 = the same radius here), each clamped
        // to >= 0; DistanceZ = context z - item z, AbsoluteZ = |that|; no context: not run
        "TresEnvQueryTest_MinDistance" => {
            let cv = ctx_points("DistanceTo", "EnvQueryContext_Querier");
            // an empty context: the test does not run (FUN_143c68d60 false -> return, items untouched)
            if cv.items.is_empty() {
                return None;
            }
            let mode = ps(p, "TestMode").map(enum_value).unwrap_or("Distance3D");
            let bounds = matches!(ps(p, "ActorDistanceMode").map(enum_value), Some("DistanceToCollisionBounds") | Some("DistanceToNavigationBounds"));
            let item_r = if bounds { w.actor(item.actor).map(|a| a.radius).unwrap_or(0.0) } else { 0.0 };
            let d = cv
                .items
                .iter()
                .enumerate()
                .map(|(i, o)| {
                    let r = if bounds { cv.radius.get(i).copied().unwrap_or(0.0) } else { 0.0 } + item_r;
                    let v = match mode {
                        "Distance2D" => Vec2::new(item.pos.x - o.pos.x, item.pos.y - o.pos.y).length() - r,
                        "DistanceZ" => o.pos.z - item.pos.z,
                        "DistanceAbsoluteZ" => (o.pos.z - item.pos.z).abs(),
                        _ => item.pos.distance(o.pos) - r,
                    };
                    v.max(0.0)
                })
                .reduce(f32::min);
            Some(d.unwrap_or(0.0))
        }
        "EnvQueryTest_Dot" => {
            // FEnvDirection [U: EnvQueryTest_Dot ctor]: LineA defaults to Rotation of the querier,
            // LineB to TwoPoints querier -> item; LineFrom / LineTo default querier / item
            let empty = Props::new();
            let line = |key: &str, default_rot: bool| -> Option<Vec3> {
                let o = pobj(p, key).unwrap_or(&empty);
                let name = |k: &str| o.get(k).and_then(|x| x.as_str()).map(|s| s.rsplit('.').next().unwrap_or(s).to_string());
                let rot_mode = match o.get("DirMode").and_then(|x| x.as_str()).map(enum_value) {
                    Some("Rotation") => true,
                    Some(_) => false,
                    None => default_rot,
                };
                if rot_mode {
                    let r = name("Rotation").unwrap_or_else(|| "EnvQueryContext_Querier".into());
                    return context(c, &r, q).rot;
                }
                let pos_of = |n: Option<String>| -> Option<Vec3> {
                    match n.as_deref() {
                        None | Some("EnvQueryContext_Item") => Some(item.pos),
                        Some(n) => context(c, n, q).items.first().map(|i| i.pos),
                    }
                };
                let fp = match name("LineFrom") {
                    None => Some(q.pos),
                    n => pos_of(n),
                }?;
                let tp = pos_of(name("LineTo"))?;
                Some((tp - fp).normalize_or_zero())
            };
            let line = |key: &str| line(key, key == "LineA");
            let (a, b) = (line("LineA")?, line("LineB")?);
            let two_d = ps(p, "TestMode").map(enum_value) == Some("Dot2D");
            let d = if two_d { a.truncate().normalize_or_zero().dot(b.truncate().normalize_or_zero()) } else { a.dot(b) };
            Some(if pb(p, "bAbsoluteValue", false) { d.abs() } else { d })
        }
        "EnvQueryTest_Trace" => {
            let cv = ctx_points("Context", "EnvQueryContext_Querier");
            let from = cv.items.first()?.pos;
            Some(if w.nav().trace_blocked(from, item.pos) { 1.0 } else { 0.0 })
        }
        "EnvQueryTest_Pathfinding" | "EnvQueryTest_PathfindingBatch" | "EnvQueryTest_Mercuna_Reachable" | "EnvQueryTest_Mercuna_NavRaycast" => {
            let cv = ctx_points("Context", "EnvQueryContext_Querier");
            let from = cv.items.first()?.pos;
            match ps(p, "TestMode").map(enum_value) {
                Some("PathCost") | Some("PathLength") => Some(w.nav().path_length(from, item.pos).unwrap_or(f32::MAX)),
                _ => Some(if w.nav().path_exists(from, item.pos) { 1.0 } else { 0.0 }),
            }
        }
        "EnvQueryTest_Project" | "EnvQueryTest_Mercuna_NavigableVolume" => Some(if w.nav().project(item.pos, 100.0, 100.0).is_some() { 1.0 } else { 0.0 }),
        // RunTest 0x1404ED0E0 [C4] (ctor 0x1404C2440: MaxDistance 1000, NavigationFilter
        // NavFilter_AIControllerDefault): the navmesh's distance to the nearest wall within
        // MaxDistance (FUN_143f00350, 50 cm query extent); no wall -> MaxDistance (float test)
        "TresEnvQueryTest_WallDistance" => {
            let max = pf(p, "MaxDistance", 1000.0);
            Some(w.nav().wall_distance(item.pos, max).unwrap_or(max))
        }
        "TresEnvQueryTest_IsCharacterState" => {
            let st = ps(p, "m_State").unwrap_or("");
            Some((c.body.view().state == st) as i32 as f32)
        }
        "TresEnvQueryTest_IsSwimmingUnderWater" => Some((c.world.player().swimming || c.body.view().swimming) as i32 as f32),
        // RunTest 0x1404E2B90 [C4] (ctor 0x1404C1850: FloorHeight 200, TestHeight 1000, TestMode
        // Navmesh, Operation Equal, bool test): the floor under the item searched down
        // TestHeight (FUN_140542040: navmesh projection from z to z - TestHeight; collision a
        // sweep); no floor -> false; else item z <op> floor z + FloorHeight
        "TresEnvQueryTest_FloorHeight" => {
            let down = param(p, "TestHeight", params, 1000.0);
            let Some(f) = w.nav().project(item.pos, 0.0, down) else { return Some(0.0) };
            let lim = f.z + param(p, "FloorHeight", params, 200.0);
            let z = item.pos.z;
            let ok = match ps(p, "Operation").map(enum_value) {
                Some("NotEqual") => z != lim,
                Some("Less") => z < lim,
                Some("LessOrEqual") => z <= lim,
                Some("Greater") => z > lim,
                Some("GreaterOrEqual") => z >= lim,
                _ => z == lim,
            };
            Some(ok as i32 as f32)
        }
        // RunTest 0x141232DE0 [C]: 1 while the item is within m_Border of the context's first
        // point (2D with m_bDist2D), 0 beyond it
        "TresNpcEnvQueryTest_BorderDistance" => {
            let o = ctx_points("m_Context", "EnvQueryContext_Querier").items.first()?.pos;
            let two_d = param(p, "m_bDist2D", params, 0.0) != 0.0 || pobj(p, "m_bDist2D").and_then(|o| o.get("DefaultValue")).and_then(|v| v.as_bool()).unwrap_or(false);
            let d = if two_d { (item.pos - o).truncate().length() } else { item.pos.distance(o) };
            let border = param(p, "m_Border", params, 0.0);
            Some(if border < d { 0.0 } else { 1.0 })
        }
        // RunTest 0x141233220 [C]: 1 while |yaw(item - camera) - camera yaw| <= m_AngleYaw (the
        // camera manager's view target +0x478), else 0
        "TresNpcEnvQueryTest_CameraAngle" => {
            let cam = w.camera();
            let to = item.pos - cam.position;
            let d = norm(to.y.atan2(to.x).to_degrees() - cam.yaw).abs();
            let lim = norm(param(p, "m_AngleYaw", params, 0.0)).abs();
            Some(if lim < d { 0.0 } else { 1.0 })
        }
        // RunTest 0x141238460 [C]: a reference yaw = querier yaw + FRand in [-m_AngleYaw,
        // m_AngleYaw] drawn once per run; value = |yaw(item - querier) - reference|
        "TresNpcEnvQueryTest_RandomAngle" => {
            let lim = param(p, "m_AngleYaw", params, 0.0);
            let reference = *c.st.eqs_draw.lock().unwrap().get_or_insert_with(|| {
                let r = c.st.eqs_rng.lock().unwrap().frand();
                q.yaw + r * (2.0 * lim) - lim
            });
            let to = item.pos - q.pos;
            Some(norm(to.y.atan2(to.x).to_degrees() - reference).abs())
        }
        // RunTest 0x141234CD0 [C]: the ITEM actor's enemy uid (vt+0xD78) == m_EnemyUID (the
        // bool test then compares with BoolValue)
        "TresNpcEnvQueryTest_DirectTargetEnemy" => {
            let uid = c.pack.enum_value("ETresEnemyUniqueID", ps(p, "m_EnemyUID").unwrap_or(""));
            Some((w.actor(item.actor).map(|a| a.uid as i64) == Some(uid)) as i32 as f32)
        }
        "TresNpcEnvQueryTest_Lead" | "TresNpcEnvQueryTest_FreeWalkAreaCheck" | "TresNpcEnvQueryTest_FreeWalkHeightCheck" => Some(0.0),
        _ => None,
    }
}

fn dist_mode(mode: &str, a: Vec3, b: Vec3) -> f32 {
    match mode {
        "Distance2D" => Vec2::new(a.x - b.x, a.y - b.y).length(),
        "DistanceZ" => a.z - b.z,
        "DistanceAbsoluteZ" => (a.z - b.z).abs(),
        _ => a.distance(b),
    }
}

/// Is the test a boolean one (BoolValue match) [U: stock defaults: Pathfinding PathExist, Trace,
/// Project; the Tres ones whose ctors call SetWorkOnFloatValues(false) (FUN_143c6c630: filter
/// Match, equation Constant): ActorOverlap, FloorHeight, Volume ...]?
fn is_bool_test(class: &str, p: &Props) -> bool {
    match class {
        "EnvQueryTest_Pathfinding" | "EnvQueryTest_PathfindingBatch" => !(matches!(ps(p, "TestMode").map(enum_value), Some("PathLength") | Some("PathCost")) || pb(p, "bWorkOnFloatValues", false)),
        "EnvQueryTest_Trace" | "EnvQueryTest_Project" | "TresEnvQueryTest_ActorOverlap" | "TresEnvQueryTest_IsCharacterState" | "TresEnvQueryTest_IsSwimmingUnderWater" | "TresEnvQueryTest_FloorHeight" | "TresEnvQueryTest_Volume" | "TresNpcEnvQueryTest_DirectTargetEnemy" | "EnvQueryTest_Mercuna_Reachable" | "EnvQueryTest_Mercuna_NavRaycast" | "EnvQueryTest_Mercuna_NavigableVolume" => true,
        _ => false,
    }
}

/// The KH3 normalisation of one test's raw values into item scores (0x143C63FC0 [C]).
pub fn normalize(p: &Props, raw: &[Option<f32>], alive: &[bool], scores: &mut [f32]) {
    let factor = pf(p, "ScoringFactor", 1.0);
    let clamp_min = ps(p, "ClampMinType").map(enum_value).unwrap_or("None");
    let clamp_max = ps(p, "ClampMaxType").map(enum_value).unwrap_or("None");
    let mut min = 0.0f32;
    let mut max = -f32::MAX;
    match clamp_min {
        "FilterThreshold" => min = pf(p, "FloatValueMin", 0.0),
        "SpecifiedValue" => min = pf(p, "ScoreClampMin", 0.0),
        _ => {}
    }
    match clamp_max {
        "FilterThreshold" => max = pf(p, "FloatValueMax", 0.0),
        "SpecifiedValue" => max = pf(p, "ScoreClampMax", 0.0),
        _ => {}
    }
    if clamp_min == "None" || clamp_max == "None" {
        for (i, r) in raw.iter().enumerate() {
            if let (true, Some(v)) = (alive[i], r) {
                if clamp_min == "None" && *v < min {
                    min = *v;
                }
                if clamp_max == "None" && max < *v {
                    max = *v;
                }
            }
        }
    }
    if min == max {
        if min == 1.0 {
            for (i, r) in raw.iter().enumerate() {
                if alive[i] && r.is_some() {
                    scores[i] += factor;
                }
            }
        }
        return;
    }
    let reference = if pb(p, "bDefineReferenceValue", false) { pf(p, "ReferenceValue", min) } else { min };
    let range = (reference - max).abs().max((reference - min).abs());
    let eq = ps(p, "ScoringEquation").map(enum_value).unwrap_or("Linear");
    for (i, r) in raw.iter().enumerate() {
        if !alive[i] {
            continue;
        }
        let s = match r {
            None => 0.0,
            Some(v) => {
                let cl = if min <= *v { if max <= *v { max } else { *v } } else { min };
                let n = (reference - cl).abs() / range;
                match eq {
                    "Square" => n * n * factor,
                    "InverseLinear" => (1.0 - n) * factor,
                    "SquareRoot" => n.sqrt() * factor,
                    "InverseSquare" => (1.0 - n) * (1.0 - n) * factor,
                    "Sine" => (n * std::f32::consts::PI).sin() * factor,
                    "Sigmoid" => factor / ((n * -12.0 + 6.0).exp() + 1.0),
                    "Constant" => {
                        if n <= 0.0 {
                            0.0
                        } else {
                            factor
                        }
                    }
                    _ => n * factor,
                }
            }
        };
        scores[i] += s;
    }
}

/// Run a query by asset name. Items in score order (best first); empty when nothing passed.
/// The query's blackboard writes are in `bb_writes` (see `run_apply`).
pub fn run(c: &Ctx, query: &str, q: &Querier, params: &Params) -> QueryResult {
    let Some(def) = c.pack.eqs.get(query) else { return QueryResult::default() };
    run_def(c, def, q, params)
}

/// `run` + the query's blackboard writes applied (the game's generators write during the query).
pub fn run_apply(c: &mut Ctx, query: &str, q: &Querier, params: &Params) -> QueryResult {
    let r = run(c, query, q, params);
    r.apply_writes(c.bb);
    r
}

pub fn run_def(c: &Ctx, def: &EqsDef, q: &Querier, params: &Params) -> QueryResult {
    let mut writes = Vec::new();
    for opt in &def.options {
        let Some(g) = &opt.generator else { continue };
        let mut items = Vec::new();
        generate(c, g, q, params, &mut items, &mut writes);
        if items.is_empty() {
            continue;
        }
        let mut alive = vec![true; items.len()];
        let mut scores = vec![0.0f32; items.len()];
        // tests: filters first when the generator auto-sorts (default), else TestOrder [U]
        let mut tests: Vec<&AuxDef> = opt.tests.iter().collect();
        let auto = pb(&g.p, "bAutoSortTests", true);
        tests.sort_by_key(|t| {
            let purpose = ps(&t.p, "TestPurpose").map(enum_value).unwrap_or("FilterAndScore");
            let order = crate::pack::pi(&t.p, "TestOrder", 0);
            if auto {
                ((purpose != "Filter") as i32, order)
            } else {
                (0, order)
            }
        });
        for t in tests {
            let p = &t.p;
            // a test's own draws are made once per RunTest (RandomAngle 0x141238460)
            *c.st.eqs_draw.lock().unwrap() = None;
            let purpose = match ps(p, "TestPurpose").map(enum_value) {
                Some("Filter") => Purpose::Filter,
                Some("Score") => Purpose::Score,
                _ => Purpose::Both,
            };
            let tv = test_values(c, t, q, &items, &alive, params);
            let mut raw: Vec<Option<f32>> = Vec::with_capacity(items.len());
            for (i, v) in tv.iter().enumerate() {
                if !alive[i] {
                    raw.push(None);
                    continue;
                }
                match v {
                    TV::Fail => {
                        alive[i] = false;
                        raw.push(None);
                    }
                    TV::Skip => raw.push(None),
                    TV::Val(x) => raw.push(Some(*x)),
                }
            }
            let boolean = is_bool_test(&t.class, p);
            if purpose != Purpose::Score {
                for (i, r) in raw.iter().enumerate() {
                    if !alive[i] {
                        continue;
                    }
                    let Some(v) = r else { continue };
                    let pass = if boolean {
                        let want = pb(p, "BoolValue", true);
                        (*v != 0.0) == want
                    } else {
                        let ft = ps(p, "FilterType").map(enum_value).unwrap_or("Range");
                        let lo = pf(p, "FloatValueMin", 0.0);
                        let hi = pf(p, "FloatValueMax", 0.0);
                        match ft {
                            "Minimum" => *v >= lo,
                            "Maximum" => *v <= hi,
                            "Match" => (*v != 0.0) == pb(p, "BoolValue", true),
                            _ => *v >= lo && *v <= hi,
                        }
                    };
                    if !pass {
                        alive[i] = false;
                    }
                }
            }
            if purpose != Purpose::Filter && !boolean {
                normalize(p, &raw, &alive, &mut scores);
            } else if purpose != Purpose::Filter && boolean {
                // a scored boolean test: match = 1, else 0, then normalised
                let want = pb(p, "BoolValue", true);
                let b: Vec<Option<f32>> = raw.iter().map(|r| r.map(|v| (((v != 0.0) == want) as i32) as f32)).collect();
                normalize(p, &b, &alive, &mut scores);
            }
        }
        let mut out: Vec<(Item, f32)> = items.into_iter().zip(scores).zip(alive).filter(|(_, a)| *a).map(|(x, _)| x).collect();
        if out.is_empty() {
            continue;
        }
        out.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        // (FOL2) the follow-type queries' winner must be walkable; the follow spot in a hallway is the member's file slot
        crate::formation::post(c, &def.name, q, &mut out);
        if out.is_empty() {
            continue;
        }
        return QueryResult { items: out, bb_writes: writes };
    }
    QueryResult { items: Vec::new(), bb_writes: writes }
}
