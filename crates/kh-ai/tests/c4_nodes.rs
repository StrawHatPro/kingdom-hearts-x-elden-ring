//! C4 (2026-10-15): the EQS generators / tests, the EQS service and the FindAttackLocation nodes,
//! the movement motion record, the focus-break bit and the Interrupt damage hook, each against
//! the rule read in the decompiled C (addresses in the code comments of kh-ai).
mod common;
use common::*;
use glam::Vec3;
use kh_ai::api::{ActorRef, AiBody, AiCommand, AiCommandStatus, BodyView, CureStyle, GimmickObstacle, UID_DONALD};
use kh_ai::bb::{BbObject, BbValue, Blackboard};
use kh_ai::bt::{Ctx, Mem, Res};
use kh_ai::controller::{AiState, FocusBreak, MotionTrack};
use kh_ai::cure::{Curer, TargetStat};
use kh_ai::eqs::{self, Item, QueryResult};
use kh_ai::pack::{AuxDef, EqsDef, EqsOptionDef, Props};
use kh_ai::AiShared;
use serde_json::{json, Value};
use std::sync::Arc;

fn props(v: Value) -> Props {
    v.as_object().cloned().unwrap()
}

fn aux(class: &str, p: Value) -> AuxDef {
    AuxDef { class: class.into(), name: String::new(), p: props(p) }
}

fn query(gen: AuxDef, tests: Vec<AuxDef>) -> EqsDef {
    EqsDef { name: "Q".into(), options: vec![EqsOptionDef { generator: Some(gen), tests }] }
}

/// A body whose damage-reaction counter the test sets (`GetDamageReactionCount`).
struct DBody<'a> {
    pawn: &'a mut Pawn,
    damage: Option<f32>,
}
impl AiBody for DBody<'_> {
    fn view(&self) -> BodyView {
        self.pawn.view
    }
    fn function(&self, name: &str) -> Option<f32> {
        match name {
            "GetDamageReactionCount" => self.damage,
            _ => None,
        }
    }
    fn command(&mut self, cmd: AiCommand) -> u32 {
        Body(self.pawn).command(cmd)
    }
    fn command_status(&self, t: u32) -> AiCommandStatus {
        let Some((ct, _, _)) = &self.pawn.cmd else { return self.pawn.status.iter().rev().find(|(x, _)| *x == t).map(|(_, s)| *s).unwrap_or_default() };
        if *ct == t {
            AiCommandStatus::Running
        } else {
            AiCommandStatus::Unknown
        }
    }
}

struct Fix {
    pack: Arc<kh_ai::AiPack>,
    bb: Blackboard,
    st: AiState,
    pawn: Pawn,
    world: World,
    shared: AiShared,
    damage: Option<f32>,
}

impl Fix {
    fn new() -> Fix {
        let pack = pack();
        let bb = Blackboard::new(&pack, "n_ex001_BB");
        let mut world = World::new();
        world.set_player(Vec3::new(-300.0, 0.0, 0.0), 0.0);
        Fix { pack, bb, st: AiState::new("n_ex001", 0, 3), pawn: Pawn::new(0, UID_DONALD, Vec3::ZERO), world, shared: AiShared::new(), damage: None }
    }
    fn with<R>(&mut self, f: impl FnOnce(&mut Ctx) -> R) -> R {
        let mut body = DBody { pawn: &mut self.pawn, damage: self.damage };
        let mut c = Ctx { pack: &self.pack, bb: &mut self.bb, body: &mut body, world: &self.world, shared: &mut self.shared, st: &mut self.st };
        f(&mut c)
    }
    fn run(&mut self, def: &EqsDef, params: &Vec<(String, f32)>) -> QueryResult {
        self.with(|c| {
            let q = kh_ai::nodes::querier(c);
            eqs::run_def(c, def, &q, params)
        })
    }
    fn target(&mut self, a: ActorRef) {
        self.bb.set_by_name("TargetActor", BbValue::Object(BbObject::Actor(a)));
    }
}

fn yaw_of(c: Vec3, p: Vec3) -> f32 {
    let d = p - c;
    d.y.atan2(d.x).to_degrees()
}

fn items(r: &QueryResult) -> Vec<Item> {
    r.items.iter().map(|(i, _)| *i).collect()
}

#[test]
fn onring3d_heights_radii_and_centres() {
    let mut f = Fix::new();
    // 0x1404C5150: heights CeilToInt((150 - -100) / 100) + 1 = 4 from -100: -100, 0, 100, 200
    // (past MaxHeight); one radius 100, CeilToInt(2 pi 100 / 100) = 7 points per ring
    let g = aux("TresEnvQueryGenerator_OnRing3D", json!({"MinRadius": {"DefaultValue": 100}, "MaxRadius": {"DefaultValue": 100}, "MinRadiusItemSpacing": {"DefaultValue": 100}, "MaxRadiusItemSpacing": {"DefaultValue": 100}, "MinHeight": {"DefaultValue": -100}, "MaxHeight": {"DefaultValue": 150}, "ItemSpacingZ": {"DefaultValue": 100}}));
    let r = f.run(&query(g, vec![]), &Vec::new());
    assert_eq!(r.items.len(), 28);
    let mut zs: Vec<i32> = items(&r).iter().map(|i| i.pos.z.round() as i32).collect();
    zs.sort();
    zs.dedup();
    assert_eq!(zs, vec![-100, 0, 100, 200]);
    // ctor 0x1404C0EA0 defaults: ItemSpacingZ 100, MinHeight 0, MaxHeight 500 -> 6 heights
    let g = aux("TresEnvQueryGenerator_OnRing3D", json!({"MinRadius": {"DefaultValue": 100}, "MaxRadius": {"DefaultValue": 100}}));
    let r = f.run(&query(g, vec![]), &Vec::new());
    let mut zs: Vec<i32> = items(&r).iter().map(|i| i.pos.z.round() as i32).collect();
    zs.sort();
    zs.dedup();
    assert_eq!(zs, vec![0, 100, 200, 300, 400, 500]);
    // the actor context adds its collision radius (ContextRadius), a point context does not
    let ring = |center: &str| aux("TresEnvQueryGenerator_OnRing", json!({"MinRadius": {"DefaultValue": 100}, "MaxRadius": {"DefaultValue": 100}, "CircleCenter": format!("/Script/TresGame.{center}"), "RadiusMode": "EEnvGeneratorOnRingRadiusMode::ContextRadius"}));
    let p = f.world.player.actor.position;
    let r = f.run(&query(ring("TresEnvQueryContext_Player"), vec![]), &Vec::new());
    assert!(items(&r).iter().all(|i| ((i.pos - p).truncate().length() - 134.0).abs() < 0.1));
    let r = f.run(&query(ring("TresEnvQueryContext_PlayerFloorPosition"), vec![]), &Vec::new());
    assert!(items(&r).iter().all(|i| ((i.pos - p).truncate().length() - 100.0).abs() < 0.1));
}

#[test]
fn context_generator_item_types_and_projection() {
    let mut f = Fix::new();
    f.world.player.actor.position.z = 50.0;
    f.world.player.floor_position.z = 50.0;
    // ctor 0x1404C0E30: Point items, TraceMode None -> the point as it is
    let g = |pd: Value, extra: Value| {
        let mut p = props(json!({"Context": "/Script/TresGame.TresEnvQueryContext_Player", "ProjectionData": pd}));
        p.extend(props(extra));
        AuxDef { class: "TresEnvQueryGenerator_Context".into(), name: String::new(), p }
    };
    let r = f.run(&query(g(json!({}), json!({})), vec![]), &Vec::new());
    assert_eq!(items(&r), vec![Item { pos: Vec3::new(-300.0, 0.0, 50.0), actor: ActorRef::None, comp: 0 }]);
    // Navigation: the floor within [z - ProjectDown, z + ProjectUp] (1024 each by default)
    let r = f.run(&query(g(json!({"TraceMode": "EEnvQueryTrace::Navigation"}), json!({})), vec![]), &Vec::new());
    assert_eq!(items(&r)[0].pos.z, 0.0);
    // no floor within ProjectDown 10: the point is dropped (Discard)
    let r = f.run(&query(g(json!({"TraceMode": "EEnvQueryTrace::Navigation", "ProjectDown": 10}), json!({})), vec![]), &Vec::new());
    assert!(r.items.is_empty());
    // Actor items: the context's actors only (Allies = the others of the team + the player)
    f.world.allies = vec![kh_ai::api::ActorView { actor: ActorRef::Member(1), position: Vec3::new(0.0, 200.0, 0.0), ..Default::default() }];
    let allies = AuxDef { class: "TresEnvQueryGenerator_Context".into(), name: String::new(), p: props(json!({"Context": "/Script/TresGame.TresEnvQueryContext_Allies", "ContextItemType": "/Script/AIModule.EnvQueryItemType_Actor", "ItemType": "/Script/AIModule.EnvQueryItemType_Actor"})) };
    let r = f.run(&query(allies, vec![]), &Vec::new());
    let mut a: Vec<ActorRef> = items(&r).iter().map(|i| i.actor).collect();
    a.sort();
    assert_eq!(a, vec![ActorRef::Player, ActorRef::Member(1)]);
    let floor = AuxDef { class: "TresEnvQueryGenerator_Context".into(), name: String::new(), p: props(json!({"Context": "/Script/TresGame.TresEnvQueryContext_PlayerFloorPosition", "ContextItemType": "/Script/AIModule.EnvQueryItemType_Actor"})) };
    assert!(f.run(&query(floor, vec![]), &Vec::new()).items.is_empty());
}

#[test]
fn side_move_points_and_side_flip() {
    let mut f = Fix::new();
    f.world.add_dummy(1, Vec3::new(500.0, 0.0, 0.0));
    f.target(ActorRef::Enemy(1));
    f.bb.set_by_name("SideMoveDirction", BbValue::Float(1.0));
    let g = aux("TresNpcEnvQueryGenerator_SideMove", json!({}));
    let r = f.run(&query(g.clone(), vec![]), &Vec::new());
    // 0x141225520: t = clamp((500 - (300 + 34 + 50)) / 100, -1, 1) = 1 -> v = unit(1, 1, 0);
    // the querier's location, then s = 34 + 70, +100, +100 along v
    let pts = items(&r);
    assert_eq!(pts.len(), 4);
    assert_eq!(pts.iter().filter(|i| i.pos.truncate().length() < 0.01).count(), 1);
    for s in [104.0f32, 204.0, 304.0] {
        let want = Vec3::new(s * 0.70710677, s * 0.70710677, 0.0);
        assert!(pts.iter().any(|i| i.pos.distance(want) < 0.05), "missing {want:?} in {pts:?}");
    }
    assert!(r.bb_writes.is_empty());
    // facing -90: the side point (yaw 90) is 180 deg off -> the side flips and is written back
    f.pawn.view.yaw = -90.0;
    let r = f.run(&query(g.clone(), vec![]), &Vec::new());
    assert_eq!(r.bb_writes, vec![("SideMoveDirction".to_string(), BbValue::Float(-1.0))]);
    assert!(items(&r).iter().any(|i| i.pos.distance(Vec3::new(104.0 * 0.70710677, -104.0 * 0.70710677, 0.0)) < 0.05));
    // no target point: only the querier's location
    f.bb.set_by_name("TargetActor", BbValue::Object(BbObject::None));
    let r = f.run(&query(g, vec![]), &Vec::new());
    assert_eq!(r.items.len(), 1);
}

#[test]
fn actor_overlap_gimmicks_and_the_box_prefilter() {
    let mut f = Fix::new();
    // items: a ring of 8 around the querier at radius 150 (x from -150 to 150)
    let ring = aux("TresEnvQueryGenerator_OnRing", json!({"MinRadius": {"DefaultValue": 150}, "MaxRadius": {"DefaultValue": 150}, "MinRadiusItemSpacing": {"DefaultValue": 118}, "MaxRadiusItemSpacing": {"DefaultValue": 118}}));
    let overlap = aux("TresEnvQueryTest_ActorOverlap", json!({"Context": "/Script/TresGame.TresEnvQueryContext_GimmickObstacles", "bUseGimmickObstacle": true, "TestPurpose": "EEnvTestPurpose::Filter", "BoolValue": {"DefaultValue": false}}));
    let all = f.run(&query(ring.clone(), vec![]), &Vec::new()).items.len();
    assert_eq!(all, 8);
    // no obstacle: the test does not run
    assert_eq!(f.run(&query(ring.clone(), vec![overlap.clone()]), &Vec::new()).items.len(), 8);
    // an obstacle at (200, 0): r 50 (+ the querier's 34): the item at (150, 0) overlaps
    // (dz = |0 - (0 + 100)| <= 100)
    f.world.gimmicks = vec![GimmickObstacle { base: Vec3::new(200.0, 0.0, 0.0), radius: 50.0, half_height: 100.0 }];
    let r = f.run(&query(ring.clone(), vec![overlap.clone()]), &Vec::new());
    assert_eq!(r.items.len(), 7);
    assert!(items(&r).iter().all(|i| i.pos.distance(Vec3::new(150.0, 0.0, 0.0)) > 1.0));
    // the same obstacle 230 cm out: 80 from the item (< 84) but outside the items' box widened
    // by its own radius (150 + 50 < 230 - 0): dropped by the prefilter, nothing overlaps
    f.world.gimmicks = vec![GimmickObstacle { base: Vec3::new(230.0, 0.0, 0.0), radius: 50.0, half_height: 100.0 }];
    assert_eq!(f.run(&query(ring, vec![overlap]), &Vec::new()).items.len(), 8);
}

#[test]
fn distance_to_path_floor_height_volume_and_wall_distance() {
    let mut f = Fix::new();
    let ring = aux("TresEnvQueryGenerator_OnRing", json!({"MinRadius": {"DefaultValue": 100}, "MaxRadius": {"DefaultValue": 300}, "MinRadiusItemSpacing": {"DefaultValue": 200}, "MaxRadiusItemSpacing": {"DefaultValue": 200}}));
    // DistanceToPath: the path from the querier to the player (-300, 0): items within 50 cm of it
    let dtp = aux("TresEnvQueryTest_DistanceToPath", json!({"PathEnd": "/Script/TresGame.TresEnvQueryContext_Player", "TestPurpose": "EEnvTestPurpose::Filter", "FilterType": "EEnvTestFilterType::Maximum", "FloatValueMax": {"DefaultValue": 50}}));
    let r = f.run(&query(ring.clone(), vec![dtp.clone()]), &Vec::new());
    assert!(!r.items.is_empty());
    for i in items(&r) {
        assert!(i.pos.x <= 0.0 && i.pos.x >= -300.0 && i.pos.y.abs() <= 50.0, "{i:?}");
    }
    // no path: every item is skipped (passes)
    let n = f.run(&query(ring.clone(), vec![]), &Vec::new()).items.len();
    f.world.nav.no_paths = true;
    assert_eq!(f.run(&query(ring.clone(), vec![dtp]), &Vec::new()).items.len(), n);
    f.world.nav.no_paths = false;
    // FloorHeight (0x1404E2B90, ctor: FloorHeight 200, TestHeight 1000): z <= floor + 200
    let fh = |h: f32| {
        let g = aux("TresEnvQueryGenerator_OnRing3D", json!({"MinRadius": {"DefaultValue": 100}, "MaxRadius": {"DefaultValue": 100}, "MinHeight": {"DefaultValue": h}, "MaxHeight": {"DefaultValue": h}}));
        let t = aux("TresEnvQueryTest_FloorHeight", json!({"Operation": "EArithmeticKeyOperation::LessOrEqual", "TestPurpose": "EEnvTestPurpose::Filter"}));
        query(g, vec![t])
    };
    assert_eq!(f.run(&fh(150.0), &Vec::new()).items.len(), 7);
    assert!(f.run(&fh(250.0), &Vec::new()).items.is_empty());
    // no floor within TestHeight 1000 below -> false
    assert!(f.run(&fh(1500.0), &Vec::new()).items.is_empty());
    // Volume (0x1404EC590): no volume + bSkipIfNoVolumeFound (ctor on) -> not run; off -> all
    // false; a TresBattleVolume box -> only the items inside
    let vol = |skip: bool| aux("TresEnvQueryTest_Volume", json!({"VolumeClass": "/Script/TresGame.TresBattleVolume", "bSkipIfNoVolumeFound": skip, "TestPurpose": "EEnvTestPurpose::Filter"}));
    assert_eq!(f.run(&query(ring.clone(), vec![vol(true)]), &Vec::new()).items.len(), n);
    assert!(f.run(&query(ring.clone(), vec![vol(false)]), &Vec::new()).items.is_empty());
    f.world.volumes = vec![("TresBattleVolume".into(), Vec3::new(0.0, -1000.0, -100.0), Vec3::new(1000.0, 1000.0, 100.0))];
    let r = f.run(&query(ring.clone(), vec![vol(false)]), &Vec::new());
    assert!(!r.items.is_empty() && items(&r).iter().all(|i| i.pos.x > 0.0));
    // WallDistance (0x1404ED0E0, ctor MaxDistance 1000): no wall -> 1000
    let wd = aux("TresEnvQueryTest_WallDistance", json!({"TestPurpose": "EEnvTestPurpose::Filter", "FilterType": "EEnvTestFilterType::Minimum", "FloatValueMin": {"DefaultValue": 999}}));
    assert_eq!(f.run(&query(ring.clone(), vec![wd.clone()]), &Vec::new()).items.len(), n);
    f.world.nav.wall = Some(200.0);
    assert!(f.run(&query(ring, vec![wd]), &Vec::new()).items.is_empty());
}

fn ring8(center: &str, radius: f32) -> AuxDef {
    let sp = std::f32::consts::TAU * radius / 8.0;
    aux("TresEnvQueryGenerator_OnRing", json!({"MinRadius": {"DefaultValue": radius}, "MaxRadius": {"DefaultValue": radius}, "MinRadiusItemSpacing": {"DefaultValue": sp}, "MaxRadiusItemSpacing": {"DefaultValue": sp}, "CircleCenter": format!("/Script/TresGame.{center}")}))
}

#[test]
fn around_reference_direction() {
    let mut f = Fix::new();
    f.world.add_dummy(1, Vec3::ZERO);
    f.target(ActorRef::Enemy(1));
    let around = aux("TresNpcEnvQueryTest_Around", json!({"m_Context": "/Script/TresGame.TresEnvQueryContext_Target", "m_Angle": {"DefaultValue": 180}, "m_WayAngle": {"DefaultValue": 45}, "TestPurpose": "EEnvTestPurpose::Score"}));
    let q = query(ring8("TresEnvQueryContext_Target", 300.0), vec![around]);
    // 0x1412324D0: desired = 0 - 180; the querier at (0, 300): bearing 90, diff 90 >= 45 ->
    // reference = 90 + 45 = 135: the best item lies at 135 deg around the target
    f.pawn.view.position = Vec3::new(0.0, 300.0, 0.0);
    let best = f.run(&q, &Vec::new()).best().unwrap();
    assert!((yaw_of(Vec3::ZERO, best.pos) - 135.0).abs() < 0.5, "{best:?}");
    // within 45 of desired (bearing ~170.5): the reference is desired itself (180)
    f.pawn.view.position = Vec3::new(-300.0, 50.0, 0.0);
    let best = f.run(&q, &Vec::new()).best().unwrap();
    assert!(kh_ai::nodes::validate::norm(yaw_of(Vec3::ZERO, best.pos) - 180.0).abs() < 0.5, "{best:?}");
}

#[test]
fn escape_direction_target_fallback_and_failure() {
    let mut f = Fix::new();
    let esc = |to_target: bool| aux("TresNpcEnvQueryTest_Escape", json!({"EscapeContext": "/Script/TresGame.TresEnvQueryContext_Enemies", "m_Radius": {"DefaultValue": 500}, "m_bEscapeToTargetActor": {"DefaultValue": to_target}, "TestPurpose": "EEnvTestPurpose::Score", "ScoringEquation": "EEnvTestScoreEquation::InverseSquare"}));
    let q = |t| query(ring8("TresEnvQueryContext_Querier", 400.0), vec![esc(t)]);
    // an enemy 200 cm ahead (gap 200 - (50 + 34) <= 500): escape straight back (yaw 180)
    f.world.add_dummy(1, Vec3::new(200.0, 0.0, 0.0));
    let best = f.run(&q(false), &Vec::new()).best().unwrap();
    assert!(kh_ai::nodes::validate::norm(yaw_of(Vec3::ZERO, best.pos) - 180.0).abs() < 0.5);
    // nobody within reach, no target: every item fails (the query has no result)
    f.world.dummies[0].position = Vec3::new(5000.0, 0.0, 0.0);
    assert!(f.run(&q(false), &Vec::new()).items.is_empty());
    // with m_bEscapeToTargetActor: away from TargetActor (at +Y): yaw -90
    f.world.add_dummy(2, Vec3::new(0.0, 2000.0, 0.0));
    f.target(ActorRef::Enemy(2));
    let best = f.run(&q(true), &Vec::new()).best().unwrap();
    assert!((yaw_of(Vec3::ZERO, best.pos) + 90.0).abs() < 0.5);
}

#[test]
fn run_eqs_service_override_rule_and_invalidation() {
    let mut f = Fix::new();
    f.world.add_dummy(1, Vec3::new(400.0, 0.0, 0.0));
    // InitializeFromAsset 0x14049F310: an override -> it runs; the result next tick (pending)
    let (svc, _) = kh_ai::nodes::make_service("TresBTService_RunEQSQuery", &props(json!({"EQSQueryOverride": "/Game/Blueprints/Npc/Common/EQS/TargetSelection/EQ_NpcTarget_LockOnTarget.EQ_NpcTarget_LockOnTarget", "bUseBlackboardQuery": false, "EQSQuery": {"SelectedKeyName": "ActionEQSQuery"}, "Result": {"SelectedKeyName": "LockOnTargetComponent"}})), &f.pack.clone());
    f.with(|c| svc.tick(c, &mut Mem::default(), 0.05));
    assert_eq!(f.st.pending_bb.len(), 1);
    assert_eq!(f.st.pending_bb[0].0, "LockOnTargetComponent");
    // no override: the blackboard key's query (bUseBlackboardQuery forced on); with
    // bInvalidateBlackboardKeyOnFailure and nothing found, the key is cleared
    f.st.pending_bb.clear();
    f.world.dummies.clear();
    f.bb.set_by_name("ActionEQSQuery", BbValue::Object(BbObject::Asset("EQ_NpcTarget_LockOnTarget".into())));
    let (svc, _) = kh_ai::nodes::make_service("TresBTService_RunEQSQuery", &props(json!({"bUseBlackboardQuery": false, "bInvalidateBlackboardKeyOnFailure": true, "EQSQuery": {"SelectedKeyName": "ActionEQSQuery"}, "Result": {"SelectedKeyName": "LockOnTargetComponent"}})), &f.pack.clone());
    f.with(|c| svc.tick(c, &mut Mem::default(), 0.05));
    assert_eq!(f.st.pending_bb, vec![("LockOnTargetComponent".to_string(), BbValue::Object(BbObject::None))]);
}

#[test]
fn find_attack_location_params_and_results() {
    let mut f = Fix::new();
    let d = f.pack.class("n_ex001_Attack1_Beat").unwrap();
    let check = kh_ai::nodes::validate::ActionCheck::from_class(d.native.as_deref().unwrap(), &d.p);
    let t = kh_ai::api::ActorView { half_height: 90.0, ..Default::default() };
    let ps: std::collections::BTreeMap<String, f32> = check.attack_query_params(Some(&t)).into_iter().collect();
    // 0x140438C90 with Beat (MaxDistance 200, MaxHeight 120, melee MinHeight 0, refinement 50)
    for (k, v) in [("Min Distance", 50.0), ("Max Distance", 150.0), ("Min Height", -120.0), ("Max Height", 0.0), ("Min Filter Height", -130.0), ("Max Filter Height", 70.0), ("Min Item Spacing", 125.0), ("Max Item Spacing", 175.0), ("MinXYItemSpacing", 500.0), ("MaxXYItemSpacing", 700.0), ("ZItemSpacing", 400.0)] {
        assert!((ps[k] - v).abs() < 1e-3, "{k} = {} (want {v})", ps[k]);
    }
    // the task: an attack with a query -> InProgress, the location next tick
    f.world.add_dummy(1, Vec3::new(600.0, 0.0, 0.0));
    f.target(ActorRef::Enemy(1));
    let (task, _) = kh_ai::nodes::make_task("TresBTTask_FindAttackLocation", &props(json!({"AttackDefinition": {"SelectedKeyName": "ActiveAction"}, "Result": {"SelectedKeyName": "Destination"}})), &f.pack.clone());
    let mut m = Mem::default();
    f.bb.set_by_name("ActiveAction", BbValue::Class(Some("n_ex001_Attack4_MagFire".into())));
    assert_eq!(f.with(|c| task.execute(c, &mut m)), Res::InProgress);
    assert_eq!(f.with(|c| task.tick(c, &mut m, 1.0 / 60.0)), Some(Res::Succeeded));
    let dest = f.bb.get_by_name("Destination").cloned();
    assert!(matches!(dest, Some(BbValue::Vector(v)) if kh_ai::bb::is_valid_location(v)));
    // no m_ActionEQSQuery (MagCure): the pawn's location at once
    f.bb.set_by_name("ActiveAction", BbValue::Class(Some("n_ex001_Action2_MagCure".into())));
    let mut m = Mem::default();
    assert_eq!(f.with(|c| task.execute(c, &mut m)), Res::Succeeded);
    assert_eq!(f.bb.get_by_name("Destination"), Some(&BbValue::Vector(Vec3::new(0.0, 0.0, f.pawn.view.half_height))));
    // not an attack definition -> Failed
    f.bb.set_by_name("ActiveAction", BbValue::Class(Some("n_ex001_Action_PlayCameraAnim".into())));
    assert_eq!(f.with(|c| task.execute(c, &mut Mem::default())), Res::Failed);
}

#[test]
fn validate_action_vector_target_passes() {
    let mut f = Fix::new();
    // the vector variants vt+0x3E0 / 0x3F0 / 0x400 = 0x14041F1E0 (`mov al,1`): even an invalid
    // vector passes all three checks
    let (deco, _) = kh_ai::nodes::make_decorator("TresBTDecorator_ValidateAction", &props(json!({"bUseBlackboardQuery": false, "ActionDefinitionOverride": "/Game/Blueprints/Npc/n_ex001/Action/n_ex001_Attack1_Beat.n_ex001_Attack1_Beat_C", "Target": {"SelectedKeyName": "Destination"}})), &f.pack.clone());
    assert!(f.with(|c| deco.calc(c, &mut Mem::default())));
}

#[test]
fn motion_record_rules() {
    // FUN_140591690: moving = some |v| > 0.0001 cm/s; a = dt / 3
    let mut m = MotionTrack::default();
    let dt = 1.0 / 60.0;
    m.step(Vec3::ZERO, dt, false);
    m.step(Vec3::ZERO, dt, false);
    assert!((m.time_motionless - 2.0 * dt).abs() < 1e-6);
    m.step(Vec3::new(0.001, 0.0, 0.0), dt, false);
    assert_eq!(m.time_motionless, 0.0);
    assert!((m.time_moving - dt).abs() < 1e-6);
    assert!((m.avg_dir.x - dt / 3.0).abs() < 1e-6);
    // swimming at the surface: vertical bobbing alone does not move
    let mut s = MotionTrack::default();
    s.step(Vec3::ZERO, dt, true);
    s.step(Vec3::new(0.0, 0.0, 5.0), dt, true);
    assert!(s.time_motionless > dt * 1.5);
}

#[test]
fn focus_break_and_focus_items() {
    let mut b = FocusBreak::default();
    b.observe(50, 100);
    assert!(!b.on);
    b.observe(0, 100);
    assert!(b.on);
    b.observe(60, 100);
    assert!(b.on);
    b.observe(100, 100);
    assert!(!b.on);
    let pack = pack();
    let mk = |style, focus_break, item_up| Curer { pack: &pack, style, in_battle: true, can_cure: false, cure_amount: 0, commands: vec!["TRES_CMD_KIND_FOCUSSUPPLEMENT".into(), "TRES_CMD_KIND_HIFOCUSSUPPLEMENT".into()], item_up, focus_break };
    let player = |f: i32| TargetStat { actor: ActorRef::Player, position: Vec3::ZERO, hp: 100, hp_max: 100, mp: Some((10, 10)), mp_charging: false, mp_charge_left: f32::INFINITY, dying: false, bad_status: false, is_player: true, focus: Some((f, 100)) };
    const NONE: &str = "TRES_CMD_KIND_NONE";
    // PINCH = the focus-break bit (0x140545D70)
    assert!(mk(CureStyle::Pinch, true, 0).focus_need(&player(80), NONE));
    assert!(!mk(CureStyle::Pinch, false, 0).focus_need(&player(0), NONE));
    // FUN_14125ef60: missing 90 -> HIFOCUS (100 % covers it) beats FOCUS (40 %)
    assert_eq!(mk(CureStyle::Always, false, 0).focus_pick(&player(10)), "TRES_CMD_KIND_HIFOCUSSUPPLEMENT");
    // missing 30: FOCUS covers with the smaller overshoot
    assert_eq!(mk(CureStyle::Always, false, 0).focus_pick(&player(70)), "TRES_CMD_KIND_FOCUSSUPPLEMENT");
    // ALWAYS: focus% <= 24 and the asked command must be the fitting one
    assert!(mk(CureStyle::Always, false, 0).focus_need(&player(10), NONE));
    assert!(mk(CureStyle::Always, false, 0).focus_need(&player(10), "TRES_CMD_KIND_HIFOCUSSUPPLEMENT"));
    assert!(!mk(CureStyle::Always, false, 0).focus_need(&player(10), "TRES_CMD_KIND_FOCUSSUPPLEMENT"));
}

#[test]
fn interrupt_damage_reaction_restarts_the_branch() {
    use kh_ai::bt::{BtComponent, Library};
    let base = pack();
    // Selector [ (Interrupt, LowerPriority) Wait 1 s, Wait 100 s ]
    let root = json!({"class": "BTComposite_Selector", "name": "S", "p": {}, "services": [], "children": [
        {"decorators": [{"class": "TresBTDecorator_Interrupt", "name": "I", "p": {"FlowAbortMode": "EBTFlowAbortMode::LowerPriority"}}], "ops": [], "node": {"class": "BTTask_Wait", "name": "A", "p": {"WaitTime": 1.0}, "services": [], "children": []}},
        {"decorators": [], "ops": [], "node": {"class": "BTTask_Wait", "name": "B", "p": {"WaitTime": 100.0}, "services": [], "children": []}}
    ]});
    let tree = json!({"path": "/Game/T", "blackboard": "n_ex001_BB", "root_decorators": [], "root_ops": [], "root": root});
    let index = json!({"format": "kh3-party-ai/1", "roots": {}, "assets": {"T": {"kind": "tree", "file": "trees/T.json"}}});
    let mut p = kh_ai::AiPack::load(&mut |rel| match rel {
        "index.json" => Some(index.to_string()),
        "trees/T.json" => Some(tree.to_string()),
        _ => None,
    })
    .unwrap();
    p.blackboards = base.blackboards.clone();
    let pack = Arc::new(p);
    let mut bt = BtComponent::new(Arc::new(Library::compile(&pack)), "T");
    let mut f = Fix::new();
    f.pack = pack.clone();
    f.damage = Some(0.0);
    let dt = 1.0 / 60.0;
    let step = |f: &mut Fix, bt: &mut BtComponent, n: i32| {
        for _ in 0..n {
            f.st.time += dt as f64;
            f.with(|c| bt.tick(c, dt));
        }
    };
    step(&mut f, &mut bt, 10);
    // not interrupted: DenyOnNormalExecution keeps A shut
    assert!(bt.active_task().contains('B'), "{}", bt.active_task());
    // an accepted damage reaction (0x1405BB850 -> 0x14054ACD0 -> 0x143C3DC20): A runs
    f.damage = Some(1.0);
    step(&mut f, &mut bt, 2);
    assert!(bt.active_task().contains('A'), "{}", bt.active_task());
    // another one 0.8 s in restarts A (RequestExecution is unconditional): still A at 1.2 s
    step(&mut f, &mut bt, 48);
    f.damage = Some(2.0);
    step(&mut f, &mut bt, 24);
    assert!(bt.active_task().contains('A'), "{}", bt.active_task());
    // A ends: its deactivation clears the byte (0x143C491A0) -> back to B
    step(&mut f, &mut bt, 60);
    assert!(bt.active_task().contains('B'), "{}", bt.active_task());
}
