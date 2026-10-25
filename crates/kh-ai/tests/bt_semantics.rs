//! Behaviour-tree semantics on small hand-made trees (stock UE 4.17 rules as the runtime models
//! them), and one EQS query of the game scored by hand.

mod common;
use glam::Vec3;
use kh_ai::bb::{BbValue, Blackboard};
use kh_ai::bt::{BtComponent, Ctx, Library};
use kh_ai::controller::AiState;
use kh_ai::*;
use serde_json::{json, Value};
use std::sync::Arc;

fn deco(class: &str, p: Value) -> Value {
    json!({"class": class, "name": class, "p": p})
}
fn child(node: Value, decorators: Vec<Value>) -> Value {
    json!({"decorators": decorators, "ops": [], "node": node})
}
fn comp(class: &str, ret: Option<&str>, children: Vec<Value>) -> Value {
    let mut p = json!({});
    if let Some(r) = ret {
        p["ReturnCondition"] = json!(format!("ETresCustomCompositeReturnCondition::{r}"));
    }
    json!({"class": class, "name": format!("{class}_{}", children.len()), "p": p, "services": [], "children": children})
}
fn wait(t: f32, name: &str) -> Value {
    json!({"class": "BTTask_Wait", "name": name, "p": {"WaitTime": t}, "services": [], "children": []})
}
fn noop(name: &str) -> Value {
    json!({"class": "TresBTTask_Noop", "name": name, "p": {}, "services": [], "children": []})
}
fn bbkey(k: &str) -> Value {
    json!({"SelectedKeyName": k})
}

/// A pack with one tree "T" over the friends' blackboard.
fn tree_pack(root: Value) -> Arc<AiPack> {
    let base = common::pack();
    let bb = base.blackboards.clone();
    let tree = json!({"path": "/Game/T", "blackboard": "n_ex001_BB", "root_decorators": [], "root_ops": [], "root": root});
    let index = json!({"format": "kh3-party-ai/1", "roots": {}, "assets": {"T": {"kind": "tree", "file": "trees/T.json"}}});
    let mut pack = AiPack::load(&mut |rel| match rel {
        "index.json" => Some(index.to_string()),
        "trees/T.json" => Some(tree.to_string()),
        _ => None,
    })
    .unwrap();
    pack.blackboards = bb;
    Arc::new(pack)
}

struct Harness {
    pack: Arc<AiPack>,
    bt: BtComponent,
    bb: Blackboard,
    st: AiState,
    pawn: common::Pawn,
    world: common::World,
    shared: AiShared,
}

impl Harness {
    fn new(root: Value) -> Harness {
        let pack = tree_pack(root);
        let lib = Arc::new(Library::compile(&pack));
        let bt = BtComponent::new(lib, "T");
        let bb = Blackboard::new(&pack, "n_ex001_BB");
        let st = AiState {
            time: 0.0,
            rng: kh_ai::rng::Rng::new(1),
            settings: AiSettings::default(),
            stop_ai: false,
            member: 0,
            pawn: "n_ex001".into(),
            tag_cooldowns: Default::default(),
            focus: ActorRef::None,
            battle_follow_mode: false,
            attack_flow_wait: false,
            attack_flow_wait_since: 0.0,
            attack_flow_phase: false,
            attack_flow_timer: 0.0,
            pending_bb: Vec::new(),
            player_motion: Default::default(),
        self_motion: Default::default(),
        focus_break: Default::default(),
            lock_time: 0.0,
            warp_timer: 0.0,
            warp_battle_on: false,
            warp_battle_window: 0.0,
            warp_ticket: 0,
            keep_moving_until: None,
            eqs_rng: std::sync::Mutex::new(kh_ai::rng::Rng::new(7)),
            eqs_draw: std::sync::Mutex::new(None),
        };
        Harness { pack, bt, bb, st, pawn: common::Pawn::new(0, UID_DONALD, Vec3::ZERO), world: common::World::new(), shared: AiShared::new() }
    }
    fn tick(&mut self) {
        self.st.time += 1.0 / 60.0;
        let mut body = common::Body(&mut self.pawn);
        let mut c = Ctx { pack: &self.pack, bb: &mut self.bb, body: &mut body, world: &self.world, shared: &mut self.shared, st: &mut self.st };
        self.bt.tick(&mut c, 1.0 / 60.0);
    }
    fn run(&mut self, s: f32) {
        for _ in 0..(s * 60.0).round() as i32 {
            self.tick();
        }
    }
    fn task(&self) -> String {
        self.bt.active_task()
    }
    fn execs(&self, name: &str) -> usize {
        self.bt.log.iter().filter(|l| l.text.starts_with("exec") && l.text.contains(&format!(":{name} "))).count()
    }
}

#[test]
fn selector_and_sequence() {
    // selector: the first child's Blackboard decorator (HasCure is set) fails -> second child
    let root = comp(
        "TresBTComposite_Custom",
        None,
        vec![child(wait(9.0, "A"), vec![deco("BTDecorator_Blackboard", json!({"BlackboardKey": bbkey("HasCure"), "OperationType": 0}))]), child(wait(9.0, "B"), vec![])],
    );
    let mut h = Harness::new(root);
    h.run(0.1);
    assert!(h.task().contains(":B "), "{}", h.task());
    // sequence: Noop then Wait run in order; a failing child stops it
    let root = comp("TresBTComposite_Custom", Some("FirstFailure"), vec![child(noop("N"), vec![]), child(wait(9.0, "W"), vec![])]);
    let mut h = Harness::new(root);
    h.run(0.1);
    assert!(h.task().contains(":W "), "{}", h.task());
    assert_eq!(h.execs("N"), 1);
}

#[test]
fn lower_priority_abort() {
    let root = comp(
        "TresBTComposite_Custom",
        None,
        vec![
            child(wait(9.0, "A"), vec![deco("BTDecorator_Blackboard", json!({"BlackboardKey": bbkey("HasCure"), "OperationType": 0, "FlowAbortMode": "EBTFlowAbortMode::LowerPriority"}))]),
            child(wait(9.0, "B"), vec![]),
        ],
    );
    let mut h = Harness::new(root);
    h.run(0.2);
    assert!(h.task().contains(":B "));
    h.bb.set_by_name("HasCure", BbValue::Bool(true));
    h.tick();
    h.tick();
    assert!(h.task().contains(":A "), "lower-priority abort did not switch: {}", h.task());
}

/// RequestExecution 0x143C42290 [C]: a lower-priority restart is dropped when the requested
/// child's other decorators do not allow it (the running branch is not restarted), and a restart
/// that finds a task at / after the SearchEnd (the requester's next sibling) is rolled back.
#[test]
fn lower_priority_abort_checks_the_child_decorators() {
    let root = comp(
        "TresBTComposite_Custom",
        None,
        vec![
            child(
                wait(9.0, "A"),
                vec![
                    deco("BTDecorator_Blackboard", json!({"BlackboardKey": bbkey("HasCure"), "OperationType": 0, "FlowAbortMode": "EBTFlowAbortMode::LowerPriority"})),
                    deco("BTDecorator_Blackboard", json!({"BlackboardKey": bbkey("HasCombination"), "OperationType": 0})),
                ],
            ),
            child(wait(9.0, "B"), vec![]),
        ],
    );
    let mut h = Harness::new(root);
    h.bt.log_enabled = true;
    h.run(0.2);
    assert!(h.task().contains(":B "));
    h.bb.set_by_name("HasCure", BbValue::Bool(true));
    h.run(0.1);
    assert!(h.task().contains(":B "), "{}", h.task());
    assert_eq!(h.execs("B"), 1, "the running branch was restarted");
    assert!(!h.bt.log.iter().any(|l| l.text.starts_with("rollback")), "the request should have been dropped before the search");
}

#[test]
fn self_abort() {
    let root = comp(
        "TresBTComposite_Custom",
        None,
        vec![
            child(wait(9.0, "A"), vec![deco("BTDecorator_Blackboard", json!({"BlackboardKey": bbkey("HasCure"), "OperationType": 0, "FlowAbortMode": "EBTFlowAbortMode::Self"}))]),
            child(wait(9.0, "B"), vec![]),
        ],
    );
    let mut h = Harness::new(root);
    h.bb.set_by_name("HasCure", BbValue::Bool(true));
    h.run(0.2);
    assert!(h.task().contains(":A "));
    h.bb.set_by_name("HasCure", BbValue::Bool(false));
    h.tick();
    h.tick();
    assert!(h.task().contains(":B "), "self abort did not leave the branch: {}", h.task());
}

#[test]
fn cooldown_and_loop() {
    // Cooldown 1 s on the Noop; the selector falls to a 0.25 s wait meanwhile; the root loops.
    let sel = comp(
        "TresBTComposite_Custom",
        None,
        vec![child(noop("N"), vec![deco("BTDecorator_Cooldown", json!({"CoolDownTime": 1.0}))]), child(wait(0.25, "W"), vec![])],
    );
    let root = comp("TresBTComposite_Custom", Some("LastNodeCompletes"), vec![child(sel, vec![deco("BTDecorator_Loop", json!({"bInfiniteLoop": true}))])]);
    let mut h = Harness::new(root);
    h.run(2.1);
    // N at t=0, ~1.0 (after the wait that ends past the cooldown), ~2.0
    let n = h.execs("N");
    assert!((2..=3).contains(&n), "Noop ran {n} times in 2.1 s");
    // a finite loop runs its child NumLoops times
    let root = comp("TresBTComposite_Custom", Some("LastNodeCompletes"), vec![child(wait(0.1, "L"), vec![deco("BTDecorator_Loop", json!({"NumLoops": 3}))]), child(wait(9.0, "End"), vec![])]);
    let mut h = Harness::new(root);
    h.run(1.0);
    assert_eq!(h.execs("L"), 3);
    assert!(h.task().contains(":End "));
}

#[test]
fn time_limit_aborts() {
    let root = comp(
        "TresBTComposite_Custom",
        None,
        vec![child(wait(10.0, "Long"), vec![deco("BTDecorator_TimeLimit", json!({"MinTimeLimit": 0.5, "MaxTimeLimit": 0.5}))]), child(wait(10.0, "After"), vec![])],
    );
    let mut h = Harness::new(root);
    h.run(0.4);
    assert!(h.task().contains(":Long "));
    h.run(0.2);
    assert!(h.task().contains(":After "), "time limit did not abort: {}", h.task());
}

#[test]
fn random_composite_weights() {
    // TresBTComposite_Random with weights 70 / 30 picks the first child about 70 % of the time
    let rnd = json!({"class": "TresBTComposite_Random", "name": "R", "p": {"Weights": [70, 30]}, "services": [], "children": [child(noop("X"), vec![]), child(noop("Y"), vec![])]});
    let root = comp("TresBTComposite_Custom", Some("LastNodeCompletes"), vec![child(rnd, vec![deco("BTDecorator_Loop", json!({"bInfiniteLoop": true}))]), child(wait(1.0, "Z"), vec![])]);
    let mut h = Harness::new(root);
    h.bt.log_enabled = true;
    let mut x = 0;
    let mut y = 0;
    for _ in 0..400 {
        h.tick();
        x += h.bt.log.iter().filter(|l| l.text.contains(":X ")).count();
        y += h.bt.log.iter().filter(|l| l.text.contains(":Y ")).count();
        h.bt.log.clear();
    }
    let share = x as f32 / (x + y) as f32;
    println!("X {x} Y {y} share {share}");
    assert!((0.6..0.8).contains(&share), "share {share}");
}

/// `EQSQuery_NpcWarp` (FriendData.WarpQuery) scored by hand: every surviving item's score is
/// recomputed here from the asset's numbers with the KH3 equations (0x143C63FC0) and compared.
#[test]
fn eqs_npc_warp_scored_by_hand() {
    let pack = common::pack();
    let mut world = common::World::new();
    world.set_player(Vec3::ZERO, 0.0);
    world.camera.position = Vec3::new(-400.0, 0.0, 150.0);
    world.camera.yaw = 0.0;
    world.camera.pitch = 0.0;
    world.allies = vec![ActorView { actor: ActorRef::Member(1), position: Vec3::new(-100.0, -180.0, 0.0), radius: 34.0, ..Default::default() }];
    let mut pawn = common::Pawn::new(0, UID_DONALD, Vec3::new(3000.0, 0.0, 0.0));
    let mut body = common::Body(&mut pawn);
    let mut bb = Blackboard::new(&pack, "n_ex001_BB");
    let mut shared = AiShared::new();
    let mut st = AiState {
        time: 0.0,
        rng: kh_ai::rng::Rng::new(1),
        settings: AiSettings::default(),
        stop_ai: false,
        member: 0,
        pawn: "n_ex001".into(),
        tag_cooldowns: Default::default(),
        focus: ActorRef::None,
        battle_follow_mode: false,
        attack_flow_wait: false,
        attack_flow_wait_since: 0.0,
            attack_flow_phase: false,
            attack_flow_timer: 0.0,
        pending_bb: Vec::new(),
        player_motion: Default::default(),
        self_motion: Default::default(),
        focus_break: Default::default(),
        lock_time: 0.0,
        warp_timer: 0.0,
            warp_battle_on: false,
            warp_battle_window: 0.0,
        warp_ticket: 0,
            keep_moving_until: None,
            eqs_rng: std::sync::Mutex::new(kh_ai::rng::Rng::new(7)),
            eqs_draw: std::sync::Mutex::new(None),
    };
    let c = Ctx { pack: &pack, bb: &mut bb, body: &mut body, world: &world, shared: &mut shared, st: &mut st };
    let q = kh_ai::nodes::querier(&c);
    let r = kh_ai::eqs::run(&c, "EQSQuery_NpcWarp", &q, &Vec::new());
    assert!(!r.items.is_empty(), "no warp point");
    // the asset's tests (pack eqs/EQSQuery_NpcWarp.json):
    //  Dot (camera forward . camera->item): filter [-1, 0.64]; score InverseSquare, clamp min 0.64 (specified), factor 2
    //  Distance to FriendHomePosition: score InverseSquare, factor 1.5, no clamps
    //  |dz| to player: filter [-300, 300]; score InverseSquare, clamp max 300 (specified), factor 1.5
    //  MinDistance to allies (bounds) >= 100: filter
    //  ActorOverlap gimmicks: filter (none here)
    let home = Vec3::new((70f32).to_radians().cos() * 180.0, (70f32).to_radians().sin() * 180.0, 0.0);
    let cam_f = Vec3::X;
    let dots: Vec<f32> = r.items.iter().map(|(i, _)| cam_f.dot((i.pos - world.camera.position).normalize())).collect();
    let dists: Vec<f32> = r.items.iter().map(|(i, _)| i.pos.distance(home)).collect();
    let dzs: Vec<f32> = r.items.iter().map(|(i, _)| i.pos.z.abs()).collect();
    // InverseSquare with the KH3 normalisation: n = |ref - clamp(v, min, max)| / max(|ref-max|, |ref-min|), ref = min
    let inv_sq = |v: f32, min: f32, max: f32, f: f32| {
        if min == max {
            return if min == 1.0 { f } else { 0.0 };
        }
        // the fork's clamp (min first, then max; no ordering assumed)
        let cl = if min <= v { if max <= v { max } else { v } } else { min };
        let n = (min - cl).abs() / (min - max).abs().max((min - min).abs());
        (1.0 - n) * (1.0 - n) * f
    };
    // unclamped sides take the item extremes (min starts at 0 as the fork does)
    let dot_max = dots.iter().cloned().fold(-f32::MAX, f32::max);
    let dist_max = dists.iter().cloned().fold(-f32::MAX, f32::max);
    let dist_min = dists.iter().cloned().fold(0.0f32, f32::min);
    let dz_min = dzs.iter().cloned().fold(0.0f32, f32::min);
    //  MinDistance to the allies has no TestPurpose: FilterAndScore, so it also scores Linear x 1
    //  (min 0, max = the largest value) [U default purpose]; the player counts as an ally
    let mds: Vec<f32> = r.items.iter().map(|(i, _)| (i.pos.distance(world.allies[0].position) - 34.0).min(i.pos.distance(Vec3::ZERO) - 34.0)).collect();
    let md_max = mds.iter().cloned().fold(-f32::MAX, f32::max);
    for (k, (item, score)) in r.items.iter().enumerate() {
        assert!(dots[k] <= 0.64 + 1e-5, "item {k} passed the dot filter with {}", dots[k]);
        assert!(mds[k] >= 100.0 - 1e-3);
        let s = inv_sq(dots[k], 0.64, dot_max, 2.0) + inv_sq(dists[k], dist_min, dist_max, 1.5) + inv_sq(dzs[k], dz_min, 300.0, 1.5) + mds[k] / md_max;
        assert!((s - score).abs() < 1e-3, "item {k} at {:?}: engine {score} hand {s}", item.pos);
    }
    let best = r.items[0].0.pos;
    println!("warp: {} items, best {:?} score {:.3}", r.items.len(), best, r.items[0].1);
    // behind the camera line or beside it, near the home side
    assert!(cam_f.dot((best - world.camera.position).normalize()) <= 0.64);
}
