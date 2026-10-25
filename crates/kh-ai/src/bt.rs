//! The behaviour-tree component (`UBehaviorTreeComponent`, stock UE 4.17 semantics [U], with the
//! KH3 fork's stock-node changes read from the decompiled C [C]).
//!
//! Model (as UE's): trees are compiled once (execution indices in UE's order: composite, its
//! services, then per child its decorators, the task's services and the child); a controller runs
//! an instance stack (subtrees pushed by `BTTask_RunBehavior` / `TresBTTask_RunBehaviorDynamic`)
//! with persistent node memory per known instance (`KnownInstances`: cooldowns inside subtrees
//! survive re-entry), a list of active auxiliary nodes per instance (services, observing
//! decorators), and execution requests that are processed at most twice per tick (before and after
//! the active task's tick, `TickComponent`): a request runs one search from a composite to the next
//! task (`ProcessExecutionRequest`), executes it, and an instant result schedules the next request.
//!
//! Flow aborts: a decorator asks for execution (`RequestExecution(UBTDecorator*)` 0x143C42790 [C]):
//! Self -> continue at the parent with the branch Failed (only while the branch executes); Lower
//! priority -> restart the search at the decorator's child, dropped when that child's decorators
//! or a branch up to the common parent do not allow it (0x143C42290 [C]); Both -> by whether the
//! branch executes. A restart whose found task does not `TakesPriorityOver` the SearchEnd (the
//! requester's next sibling, instance first; ProcessExecutionRequest 0x143C3F8C0 [C]) is rolled back.

use crate::api::{AiBody, AiWorld};
use crate::bb::Blackboard;
use crate::controller::{AiShared, AiState};
use crate::pack::{AiPack, AuxDef, NodeDef, TreeDef};
use std::collections::BTreeMap;
use std::sync::Arc;

pub type NodeIx = u16;
pub type AuxIx = u16;

/// `EBTNodeResult`: Succeeded 0, Failed 1, Aborted 2, InProgress 3.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Res {
    Succeeded,
    Failed,
    Aborted,
    InProgress,
}

/// `EBTFlowAbortMode`: None 0, LowerPriority 1, Self 2, Both 3.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AbortMode {
    None,
    LowerPriority,
    SelfAbort,
    Both,
}

impl AbortMode {
    pub fn parse(s: Option<&str>) -> AbortMode {
        match s.map(crate::pack::enum_value) {
            Some("LowerPriority") => AbortMode::LowerPriority,
            Some("Self") => AbortMode::SelfAbort,
            Some("Both") => AbortMode::Both,
            _ => AbortMode::None,
        }
    }
    #[allow(dead_code)]
    fn observes_self(self) -> bool {
        matches!(self, AbortMode::SelfAbort | AbortMode::Both)
    }
    fn observes_lower(self) -> bool {
        matches!(self, AbortMode::LowerPriority | AbortMode::Both)
    }
}

/// Per-node, per-instance memory (UE's node memory blocks, one shape for all node kinds).
#[derive(Clone, Debug, Default)]
pub struct Mem {
    pub f: [f32; 6],
    pub i: [i32; 4],
    pub flag: u32,
    pub ticket: u32,
    pub v: glam::Vec3,
    pub actor: crate::api::ActorRef,
    /// Aux nodes: `FBTAuxiliaryMemory` NextTickRemainingTime / accumulated delta.
    pub next_tick: f32,
    pub accum: f32,
    /// Composites: `FBTCompositeMemory` CurrentChild / OverrideChild (-1 = NotInitialized).
    pub cur_child: i32,
    pub override_child: i32,
    /// Random composite: the live weights of the distribution (0x1411818E0 object).
    pub weights: Vec<f32>,
    pub search_id: u32,
    /// Free-form (EQS results waiting, names ...).
    pub s: String,
    pub initialized: bool,
}

/// What a decorator's deactivation asks of its parent composite.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DecoEffect {
    /// `SetChildOverride(ChildIndex)` (BTDecorator_Loop).
    pub repeat_child: bool,
}

/// Context handed to nodes.
pub struct Ctx<'a> {
    pub pack: &'a AiPack,
    pub bb: &'a mut Blackboard,
    pub body: &'a mut dyn AiBody,
    pub world: &'a dyn AiWorld,
    pub shared: &'a mut AiShared,
    pub st: &'a mut AiState,
}

impl Ctx<'_> {
    pub fn now(&self) -> f64 {
        self.st.time
    }
}

pub trait TaskNode: Send + Sync + std::fmt::Debug {
    /// `ExecuteTask`.
    fn execute(&self, c: &mut Ctx, m: &mut Mem) -> Res;
    /// `AbortTask`: `Aborted` at once, or `InProgress` for a latent abort (finish it from `tick`).
    fn abort(&self, _c: &mut Ctx, _m: &mut Mem) -> Res {
        Res::Aborted
    }
    /// `TickTask` for a running (or latently aborting) task: `Some(result)` = FinishLatentTask /
    /// FinishLatentAbort.
    fn tick(&self, _c: &mut Ctx, _m: &mut Mem, _dt: f32) -> Option<Res> {
        None
    }
    /// `OnTaskFinished`.
    fn on_finished(&self, _c: &mut Ctx, _m: &mut Mem, _r: Res) {}
    fn describe(&self) -> String {
        String::new()
    }
}

pub trait DecoratorNode: Send + Sync + std::fmt::Debug {
    /// `CalculateRawConditionValue` (without the inversion).
    fn calc(&self, _c: &mut Ctx, _m: &mut Mem) -> bool {
        true
    }
    /// resh: the parent composite was not already running this child (Loop's re-entry test).
    fn on_activation(&self, _c: &mut Ctx, _m: &mut Mem, _search_id: u32, _fresh: bool) {}
    fn on_deactivation(&self, _c: &mut Ctx, _m: &mut Mem, _r: Res, _search_id: u32) -> DecoEffect {
        DecoEffect::default()
    }
    fn on_processed(&self, _c: &mut Ctx, _m: &mut Mem, _r: &mut Res) {}
    /// Called when the aux node becomes relevant (registered).
    fn on_become_relevant(&self, _c: &mut Ctx, _m: &mut Mem) {}
    fn on_cease_relevant(&self, _c: &mut Ctx, _m: &mut Mem) {}
    /// `TickNode` (only when `ticks()`); `true` = `RequestExecution(this)`.
    fn tick(&self, _c: &mut Ctx, _m: &mut Mem, _dt: f32, _executing_branch: bool) -> bool {
        false
    }
    fn ticks(&self) -> bool {
        false
    }
    /// The aux node uses the interval timer (`bTickIntervals`, TimeLimit).
    fn tick_intervals(&self) -> bool {
        false
    }
    /// Blackboard key this decorator observes (`BTDecorator_BlackboardBase`); a change calls
    /// `on_key_change`, `true` = request execution.
    fn observed_key(&self) -> Option<&str> {
        None
    }
    fn on_key_change(&self, _c: &mut Ctx, _m: &mut Mem, _executing_branch: bool, _inverse: bool) -> bool {
        false
    }
    /// Does it implement anything beyond `calc` (for the reachability report).
    fn describe(&self) -> String {
        String::new()
    }
}

pub trait ServiceNode: Send + Sync + std::fmt::Debug {
    fn on_become_relevant(&self, _c: &mut Ctx, _m: &mut Mem) {}
    fn on_cease_relevant(&self, _c: &mut Ctx, _m: &mut Mem) {}
    /// `TickNode` body (the interval rescheduling is the runtime's).
    fn tick(&self, _c: &mut Ctx, _m: &mut Mem, _dt: f32) {}
    /// `OnSearchStart` (base: tick when `bCallTickOnSearchStart`).
    fn on_search_start(&self, _c: &mut Ctx, _m: &mut Mem) {}
    fn describe(&self) -> String {
        String::new()
    }
}

/// Composite logic.
#[derive(Clone, Debug)]
pub enum CompositeKind {
    /// `BTComposite_Selector` / TresBTComposite_Custom FirstSuccess (0x140469340 [C]).
    Selector,
    /// `BTComposite_Sequence` / Custom FirstFailure.
    Sequence,
    /// Custom LastNodeCompletes ("Unconditional Sequence").
    Unconditional,
    /// `TresBTComposite_Random` (0x140469390 [C]): one weighted pick; `fail_on_all` retries others
    /// after a failure.
    Random { weights: Vec<f32>, fail_on_all: bool },
    /// `BTComposite_SimpleParallel` (main task + background): not used by the friends' trees.
    SimpleParallel,
}

#[derive(Debug)]
pub enum NodeKind {
    Composite { comp: CompositeKind, children: Vec<CChild> },
    Task(Box<dyn TaskNode>),
    /// `BTTask_RunBehavior` (asset) / `TresBTTask_RunBehaviorDynamic` (tag, default asset).
    RunBehavior { asset: Option<String>, tag: Option<String> },
}

#[derive(Debug)]
pub struct CChild {
    pub decorators: Vec<AuxIx>,
    pub ops: Vec<(String, u16)>,
    pub node: NodeIx,
    /// Execution index of the child's first node (its first decorator, else the task's first
    /// service, else the child itself): `GetChildExecutionIndex(i, FirstNode)`.
    pub first_exec: u16,
}

#[derive(Debug)]
pub struct CNode {
    pub class: String,
    pub name: String,
    pub node_name: String,
    pub exec: u16,
    /// Last execution index of the subtree (`LastExecutionIndex`).
    pub last_exec: u16,
    pub parent: Option<NodeIx>,
    pub child_index: u16,
    pub services: Vec<AuxIx>,
    pub kind: NodeKind,
    pub mem: u16,
    pub implemented: bool,
}

#[derive(Debug)]
pub enum AuxKind {
    Decorator(Box<dyn DecoratorNode>),
    Service(Box<dyn ServiceNode>),
}

#[derive(Debug)]
pub struct CAux {
    pub class: String,
    pub name: String,
    pub node_name: String,
    pub exec: u16,
    /// Decorators: the composite whose child they decorate (None = root decorator).
    /// Services: the node (composite or task) they belong to.
    pub owner: Option<NodeIx>,
    pub child_index: u16,
    pub abort: AbortMode,
    pub inverse: bool,
    /// Services: interval, deviation, `bCallTickOnSearchStart`.
    pub interval: f32,
    pub deviation: f32,
    pub tick_on_search_start: bool,
    pub kind: AuxKind,
    pub mem: u16,
    pub implemented: bool,
    /// The class sets bNotifyTick (ctor `field_0x70 |= 4`) and leaves TickNode to the fork's
    /// base `FUN_140476D90` (vtable slot 0x2D8 of 111 Tres decorator classes) [C]: when
    /// FlowAbortMode (+0x84) is not None and (CalculateRawConditionValue vt+0x318 ^
    /// bInverseCondition) differs from IsExecutingBranch (0x143C3B320, child index +0x71), the node
    /// memory's +0xC is set and RequestExecution (0x143C42790) is called; BoolPropertyCheck's own
    /// TickNode 0x140476E30 is the same with its function call.
    pub cond_tick: bool,
}

#[derive(Debug)]
pub struct CTree {
    pub name: String,
    pub blackboard: Option<String>,
    pub nodes: Vec<CNode>,
    pub aux: Vec<CAux>,
    pub root_decorators: Vec<AuxIx>,
    pub root_ops: Vec<(String, u16)>,
    pub mem_slots: usize,
}

/// Every compiled tree of a pack.
#[derive(Debug, Default)]
pub struct Library {
    pub trees: BTreeMap<String, Arc<CTree>>,
}

/// The node factory (`nodes::make_*`): returns the implementation and whether the class is
/// implemented (false = a stand-in that fails / passes, listed by the reachability report).
pub struct Factory<'a> {
    pub pack: &'a AiPack,
}

// ---- compilation -----------------------------------------------------------------------------

struct Compiler<'a> {
    pack: &'a AiPack,
    nodes: Vec<CNode>,
    aux: Vec<CAux>,
    exec: u16,
    mem: u16,
}

impl Compiler<'_> {
    fn aux(&mut self, def: &AuxDef, owner: Option<NodeIx>, child_index: u16, is_service: bool) -> AuxIx {
        let exec = self.exec;
        self.exec += 1;
        let mem = self.mem;
        self.mem += 1;
        let p = &def.p;
        let (kind, implemented) = if is_service {
            let (s, ok) = crate::nodes::make_service(&def.class, p, self.pack);
            (AuxKind::Service(s), ok)
        } else {
            let (d, ok) = crate::nodes::make_decorator(&def.class, p, self.pack);
            (AuxKind::Decorator(d), ok)
        };
        let abort = crate::nodes::decorator_abort_mode(&def.class, p);
        let ix = self.aux.len() as AuxIx;
        self.aux.push(CAux {
            class: def.class.clone(),
            name: def.name.clone(),
            node_name: crate::pack::ps(p, "NodeName").unwrap_or("").to_string(),
            exec,
            owner,
            child_index,
            abort,
            inverse: crate::pack::pb(p, "bInverseCondition", false),
            interval: crate::pack::pf(p, "Interval", crate::nodes::service_default_interval(&def.class)),
            deviation: crate::pack::pf(p, "RandomDeviation", crate::nodes::service_default_deviation(&def.class)),
            tick_on_search_start: crate::pack::pb(p, "bCallTickOnSearchStart", def.class == "TresBTService_GameplayFocus"),
            cond_tick: !is_service && crate::nodes::decorator_cond_tick(&def.class),
            kind,
            mem,
            implemented,
        });
        ix
    }

    fn node(&mut self, def: &NodeDef, parent: Option<NodeIx>, child_index: u16, preassigned_services: Option<Vec<AuxIx>>) -> NodeIx {
        let exec = self.exec;
        self.exec += 1;
        let mem = self.mem;
        self.mem += 1;
        let ix = self.nodes.len() as NodeIx;
        let class = def.class.clone();
        let is_composite = matches!(class.as_str(), "BTComposite_Selector" | "BTComposite_Sequence" | "TresBTComposite_Custom" | "TresBTComposite_Random" | "BTComposite_SimpleParallel");
        self.nodes.push(CNode {
            class: class.clone(),
            name: def.name.clone(),
            node_name: crate::pack::ps(&def.p, "NodeName").unwrap_or("").to_string(),
            exec,
            last_exec: exec,
            parent,
            child_index,
            services: Vec::new(),
            kind: NodeKind::Composite { comp: CompositeKind::Selector, children: Vec::new() },
            mem,
            implemented: true,
        });
        if is_composite {
            // composite: index, then its services, then each child's decorators + child
            let services: Vec<AuxIx> = def.services.iter().map(|s| self.aux(s, Some(ix), 0, true)).collect();
            let comp = match class.as_str() {
                "BTComposite_Selector" => CompositeKind::Selector,
                "BTComposite_Sequence" => CompositeKind::Sequence,
                "BTComposite_SimpleParallel" => CompositeKind::SimpleParallel,
                "TresBTComposite_Random" => {
                    let w: Vec<f32> = crate::pack::parr(&def.p, "Weights").iter().map(|v| v.as_f64().unwrap_or(1.0) as f32).collect();
                    // 0x14046F1B0: the weights array is sized to the children, new entries 1.0
                    let mut w = w;
                    w.resize(def.children.len(), 1.0);
                    CompositeKind::Random { weights: w, fail_on_all: crate::pack::pb(&def.p, "bFailOnAll", false) }
                }
                _ => match crate::pack::ps(&def.p, "ReturnCondition").map(crate::pack::enum_value) {
                    Some("FirstFailure") => CompositeKind::Sequence,
                    Some("LastNodeCompletes") => CompositeKind::Unconditional,
                    // ctor 0x14045D030 leaves ReturnCondition 0 = FirstSuccess [C]
                    _ => CompositeKind::Selector,
                },
            };
            let mut children = Vec::new();
            for (ci, ch) in def.children.iter().enumerate() {
                let first_exec = self.exec;
                let decorators: Vec<AuxIx> = ch.decorators.iter().map(|d| self.aux(d, Some(ix), ci as u16, false)).collect();
                let child_is_composite = matches!(ch.node.class.as_str(), "BTComposite_Selector" | "BTComposite_Sequence" | "TresBTComposite_Custom" | "TresBTComposite_Random" | "BTComposite_SimpleParallel");
                let node = if child_is_composite {
                    self.node(&ch.node, Some(ix), ci as u16, None)
                } else {
                    // task: its services are numbered before the task (UE InitializeNodeHelper)
                    let placeholder_parent = Some(ix);
                    let svc_owner_ix = self.nodes.len() as NodeIx; // the task's index once pushed
                    let svcs: Vec<AuxIx> = ch.node.services.iter().map(|s| self.aux(s, Some(svc_owner_ix), 0, true)).collect();
                    self.node(&ch.node, placeholder_parent, ci as u16, Some(svcs))
                };
                children.push(CChild { decorators, ops: ch.ops.clone(), node, first_exec });
            }
            let n = &mut self.nodes[ix as usize];
            n.services = services;
            n.kind = NodeKind::Composite { comp, children };
            n.last_exec = self.exec - 1;
        } else {
            let services = preassigned_services.unwrap_or_default();
            let (kind, implemented) = match class.as_str() {
                "BTTask_RunBehavior" => (NodeKind::RunBehavior { asset: crate::pack::passet(&def.p, "BehaviorAsset"), tag: None }, true),
                "TresBTTask_RunBehaviorDynamic" | "BTTask_RunBehaviorDynamic" => (
                    NodeKind::RunBehavior { asset: crate::pack::passet(&def.p, "DefaultBehaviorAsset"), tag: crate::pack::ptag(&def.p, "InjectionTag") },
                    true,
                ),
                _ => {
                    let (t, ok) = crate::nodes::make_task(&class, &def.p, self.pack);
                    (NodeKind::Task(t), ok)
                }
            };
            let n = &mut self.nodes[ix as usize];
            n.services = services;
            n.kind = kind;
            n.implemented = implemented;
            n.last_exec = n.exec;
        }
        ix
    }
}

pub fn compile(pack: &AiPack, def: &TreeDef) -> CTree {
    let mut c = Compiler { pack, nodes: Vec::new(), aux: Vec::new(), exec: 0, mem: 0 };
    // root decorators are evaluated by the RunBehavior task that pushes the tree
    let root_decorators: Vec<AuxIx> = def.root_decorators.iter().map(|d| c.aux(d, None, 0, false)).collect();
    if let Some(root) = &def.root {
        c.node(root, None, 0, None);
    }
    CTree {
        name: def.name.clone(),
        blackboard: def.blackboard.clone(),
        nodes: c.nodes,
        aux: c.aux,
        root_decorators,
        root_ops: def.root_ops.clone(),
        mem_slots: c.mem as usize,
    }
}

impl Library {
    pub fn compile(pack: &AiPack) -> Library {
        let mut trees = BTreeMap::new();
        for (name, def) in &pack.trees {
            trees.insert(name.clone(), Arc::new(compile(pack, def)));
        }
        Library { trees }
    }
}

// ---- runtime ---------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)]
enum ActiveType {
    Composite,
    ActiveTask,
    AbortingTask,
    InactiveTask,
}

#[derive(Clone, Debug)]
struct Instance {
    tree: Arc<CTree>,
    known: usize,
    active: Option<NodeIx>,
    active_type: ActiveType,
    /// Active aux nodes in registration order.
    aux_active: Vec<AuxIx>,
}

#[derive(Clone, Debug)]
struct KnownInstance {
    tree: String,
    /// Execution indices of the RunBehavior tasks leading here (UE's FBehaviorTreeInstanceId path).
    path: Vec<u16>,
    mem: Vec<Mem>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum UpdateMode {
    Add,
    Remove,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct AuxUpdate {
    inst: usize,
    aux: AuxIx,
    mode: UpdateMode,
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
struct Request {
    inst: usize,
    composite: NodeIx,
    /// Child the request names (the finished / aborted child, or the child to restart at).
    child: i32,
    result: Res,
    try_next_child: bool,
    /// Restart: search start (instance, execution index) and end (the active node).
    search_start: Option<(usize, u16)>,
    /// `ExecutionRequest.SearchEnd` (BT component +0x2BC instance / +0x2BE execution index): for a
    /// lower-priority restart the first execution index of the requester's next sibling branch
    /// (`RequestedOn->GetChildExecutionIndex(child + 1)`, the composite's last + 1 after its last
    /// child) in the requester's instance (0x143C42290 [C]). The restart is valid only if the task
    /// it finds `TakesPriorityOver` it (ProcessExecutionRequest 0x143C3F8C0 [C]).
    search_end: Option<(usize, u16)>,
}


/// `FBTNodeIndex::TakesPriorityOver` as KH3's build compiles it (0x143C42290 / 0x143C3F8C0 [C]):
/// the instance closer to the root first, then the lower execution index.
fn takes_priority(a: (usize, u16), b: (usize, u16)) -> bool {
    if a.0 != b.0 {
        return a.0 < b.0;
    }
    a.1 < b.1
}

/// A line of the AI trace.
#[derive(Clone, Debug, Default)]
pub struct TraceLine {
    pub time: f64,
    pub text: String,
}

pub struct BtComponent {
    lib: Arc<Library>,
    stack: Vec<Instance>,
    known: Vec<KnownInstance>,
    request: Option<Request>,
    /// Updates collected by the running search (UE `SearchData.PendingUpdates`).
    pending: Vec<AuxUpdate>,
    search_id: u32,
    /// Dynamic subtree injections: gameplay tag -> tree (`m_DynamicAIBehavior`).
    pub dynamic: Vec<(String, String)>,
    /// The next task found by a search while the current one aborts latently.
    pending_task: Option<(usize, NodeIx)>,
    /// Search start (restart) carried through `get_next_child` (UE SearchData.SearchStart).
    search_start: Option<(usize, u16)>,
    pub log: Vec<TraceLine>,
    pub log_enabled: bool,
    root_name: String,
    /// Aux nodes of instances popped by the running search (OnCeaseRelevant at commit).
    popped_removes: Vec<(Arc<CTree>, usize, AuxIx)>,
    /// A task whose AbortTask answered InProgress: (tree, known instance, node, instance index).
    aborting: Option<(Arc<CTree>, usize, NodeIx, usize)>,
    last_failed: String,
}

impl std::fmt::Debug for BtComponent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BtComponent").field("root", &self.root_name).field("stack", &self.stack.len()).finish()
    }
}

/// A compiled tree's nodes by index.
macro_rules! node {
    ($self:ident, $inst:expr, $n:expr) => {
        &$self.stack[$inst].tree.nodes[$n as usize]
    };
}

impl BtComponent {
    pub fn new(lib: Arc<Library>, root: &str) -> BtComponent {
        let mut c = BtComponent {
            lib,
            stack: Vec::new(),
            known: Vec::new(),
            request: None,
            pending: Vec::new(),
            search_id: 0,
            dynamic: Vec::new(),
            pending_task: None,
            search_start: None,
            log: Vec::new(),
            log_enabled: true,
            root_name: root.to_string(),
            popped_removes: Vec::new(),
            aborting: None,
            last_failed: String::new(),
        };
        c.start();
        c
    }

    /// (Re)start the root tree (`StartTree`): drop the stack, keep nothing.
    pub fn start(&mut self) {
        self.stack.clear();
        self.known.clear();
        self.request = None;
        self.pending.clear();
        self.pending_task = None;
        if let Some(t) = self.lib.trees.get(&self.root_name).cloned() {
            let known = self.known_instance(&t.name, Vec::new(), t.mem_slots);
            self.stack.push(Instance { tree: t, known, active: None, active_type: ActiveType::Composite, aux_active: Vec::new() });
            self.request = Some(Request { inst: 0, composite: 0, child: -1, result: Res::InProgress, try_next_child: true, search_start: None, search_end: None });
        }
    }

    fn known_instance(&mut self, tree: &str, path: Vec<u16>, slots: usize) -> usize {
        if let Some(i) = self.known.iter().position(|k| k.tree == tree && k.path == path) {
            return i;
        }
        let mut mem = vec![Mem::default(); slots];
        for m in &mut mem {
            m.cur_child = -1;
            m.override_child = -1;
        }
        self.known.push(KnownInstance { tree: tree.to_string(), path, mem });
        self.known.len() - 1
    }

    fn trace(&mut self, time: f64, text: String) {
        if self.log_enabled {
            self.log.push(TraceLine { time, text });
            if self.log.len() > 256 {
                self.log.drain(0..64);
            }
        }
    }

    /// The active path: "tree/node" for every instance's active node (root first) and the task.
    pub fn active_path(&self) -> Vec<String> {
        let mut out = Vec::new();
        for inst in &self.stack {
            let mut chain = Vec::new();
            let mut cur = inst.active;
            while let Some(n) = cur {
                let node = &inst.tree.nodes[n as usize];
                let label = if node.node_name.is_empty() { node.name.clone() } else { format!("{} \"{}\"", node.name, node.node_name) };
                chain.push(label);
                cur = node.parent;
            }
            chain.reverse();
            out.push(format!("{}: {}", inst.tree.name, chain.join(" > ")));
        }
        out
    }

    /// The active task's class / name, "" when none.
    pub fn active_task(&self) -> String {
        if let Some(inst) = self.stack.last() {
            if let Some(n) = inst.active {
                let node = &inst.tree.nodes[n as usize];
                if matches!(inst.active_type, ActiveType::ActiveTask | ActiveType::AbortingTask) {
                    return format!("{}:{} [{}]", inst.tree.name, node.name, node.class);
                }
            }
        }
        String::new()
    }

    pub fn tree_names(&self) -> Vec<String> {
        self.stack.iter().map(|i| i.tree.name.clone()).collect()
    }

    // ---- memory helpers ----

    fn mem_ptr(&mut self, inst: usize, slot: u16) -> &mut Mem {
        let k = self.stack[inst].known;
        &mut self.known[k].mem[slot as usize]
    }

    fn take_mem(&mut self, inst: usize, slot: u16) -> Mem {
        std::mem::take(self.mem_ptr(inst, slot))
    }

    fn put_mem(&mut self, inst: usize, slot: u16, m: Mem) {
        *self.mem_ptr(inst, slot) = m;
    }

    // ---- aux nodes ----

    fn is_aux_active(&self, inst: usize, aux: AuxIx) -> bool {
        self.stack.get(inst).map(|i| i.aux_active.contains(&aux)).unwrap_or(false)
    }

    /// UE `AddUniqueUpdate`.
    fn add_update(&mut self, u: AuxUpdate) {
        let mut skip = false;
        let mut idx = 0;
        while idx < self.pending.len() {
            let p = self.pending[idx];
            if p.aux == u.aux && p.inst == u.inst {
                if p.mode == u.mode {
                    skip = true;
                    break;
                }
                skip = p.mode == UpdateMode::Remove || u.mode == UpdateMode::Remove;
                self.pending.remove(idx);
                continue;
            }
            idx += 1;
        }
        if !skip && u.mode == UpdateMode::Remove && !self.is_aux_active(u.inst, u.aux) {
            skip = true;
        }
        if !skip {
            self.pending.push(u);
        }
    }

    /// `UnregisterAuxNodesUpTo(index)`: remove every active aux node `index` takes priority over.
    fn unregister_aux_up_to(&mut self, index: (usize, u16)) {
        let mut ups = Vec::new();
        for (ii, inst) in self.stack.iter().enumerate() {
            for &a in &inst.aux_active {
                let ai = (ii, inst.tree.aux[a as usize].exec);
                if takes_priority(index, ai) {
                    ups.push(AuxUpdate { inst: ii, aux: a, mode: UpdateMode::Remove });
                }
            }
        }
        for u in ups {
            self.add_update(u);
        }
    }

    /// `ApplySearchUpdates`.
    fn apply_updates(&mut self, c: &mut Ctx) {
        let ups = std::mem::take(&mut self.pending);
        for u in ups {
            if u.inst >= self.stack.len() {
                continue;
            }
            let tree = self.stack[u.inst].tree.clone();
            let a = &tree.aux[u.aux as usize];
            match u.mode {
                UpdateMode::Remove => {
                    let list = &mut self.stack[u.inst].aux_active;
                    if let Some(p) = list.iter().position(|x| *x == u.aux) {
                        list.remove(p);
                        let mut m = self.take_mem(u.inst, a.mem);
                        match &a.kind {
                            AuxKind::Decorator(d) => d.on_cease_relevant(c, &mut m),
                            AuxKind::Service(s) => s.on_cease_relevant(c, &mut m),
                        }
                        self.put_mem(u.inst, a.mem, m);
                    }
                }
                UpdateMode::Add => {
                    let list = &mut self.stack[u.inst].aux_active;
                    if !list.contains(&u.aux) {
                        list.push(u.aux);
                        let mut m = self.take_mem(u.inst, a.mem);
                        match &a.kind {
                            AuxKind::Decorator(d) => d.on_become_relevant(c, &mut m),
                            AuxKind::Service(s) => s.on_become_relevant(c, &mut m),
                        }
                        self.put_mem(u.inst, a.mem, m);
                    }
                }
            }
        }
    }

    // ---- decorators ----

    fn deco_can_execute(&mut self, c: &mut Ctx, inst: usize, aux: AuxIx) -> bool {
        let tree = self.stack[inst].tree.clone();
        let a = &tree.aux[aux as usize];
        let mut m = self.take_mem(inst, a.mem);
        let raw = match &a.kind {
            AuxKind::Decorator(d) => d.calc(c, &mut m),
            AuxKind::Service(_) => true,
        };
        self.put_mem(inst, a.mem, m);
        if raw == a.inverse {
            self.last_failed = format!("{} {}", a.class, a.node_name);
        }
        raw != a.inverse
    }

    /// `DoDecoratorsAllowExecution` for child `child` of composite `comp` (or the root
    /// decorators when `comp` is None).
    fn decorators_allow(&mut self, c: &mut Ctx, inst: usize, decos: &[AuxIx], ops: &[(String, u16)]) -> bool {
        if decos.is_empty() {
            return true;
        }
        if ops.is_empty() {
            for &d in decos {
                if !self.deco_can_execute(c, inst, d) {
                    return false;
                }
            }
            return true;
        }
        // prefix logic (And / Or / Not / Test), short-circuit like UE's forced results
        let mut pos = 0usize;
        self.eval_ops(c, inst, decos, ops, &mut pos, false)
    }

    fn eval_ops(&mut self, c: &mut Ctx, inst: usize, decos: &[AuxIx], ops: &[(String, u16)], pos: &mut usize, skip: bool) -> bool {
        let Some((op, n)) = ops.get(*pos).cloned() else { return true };
        *pos += 1;
        match op.as_str() {
            "Test" => {
                if skip {
                    return true;
                }
                match decos.get(n as usize) {
                    Some(&d) => self.deco_can_execute(c, inst, d),
                    None => false,
                }
            }
            "Not" => !self.eval_ops(c, inst, decos, ops, pos, skip),
            "And" => {
                let mut r = true;
                for _ in 0..n {
                    let v = self.eval_ops(c, inst, decos, ops, pos, skip || !r);
                    r = r && v;
                }
                r
            }
            "Or" => {
                let mut r = false;
                for _ in 0..n {
                    let v = self.eval_ops(c, inst, decos, ops, pos, skip || r);
                    r = r || v;
                }
                r
            }
            _ => true,
        }
    }

    fn child_decos(&self, inst: usize, comp: NodeIx, child: usize) -> (Vec<AuxIx>, Vec<(String, u16)>) {
        match &node!(self, inst, comp).kind {
            NodeKind::Composite { children, .. } => children.get(child).map(|ch| (ch.decorators.clone(), ch.ops.clone())).unwrap_or_default(),
            _ => (Vec::new(), Vec::new()),
        }
    }

    // ---- composite flow ----

    fn children_len(&self, inst: usize, comp: NodeIx) -> usize {
        match &node!(self, inst, comp).kind {
            NodeKind::Composite { children, .. } => children.len(),
            _ => 0,
        }
    }

    fn child_node(&self, inst: usize, comp: NodeIx, child: usize) -> NodeIx {
        match &node!(self, inst, comp).kind {
            NodeKind::Composite { children, .. } => children[child].node,
            _ => 0,
        }
    }

    fn child_first_exec(&self, inst: usize, comp: NodeIx, child: usize) -> u16 {
        match &node!(self, inst, comp).kind {
            NodeKind::Composite { children, .. } => match children.get(child) {
                Some(ch) => ch.first_exec,
                None => node!(self, inst, comp).last_exec + 1,
            },
            _ => 0,
        }
    }

    /// The composite's next-child handler (`OnNextChild` delegate).
    fn next_child_handler(&mut self, c: &mut Ctx, inst: usize, comp: NodeIx, prev: i32, last: Res) -> i32 {
        let n_children = self.children_len(inst, comp) as i32;
        let kind = match &node!(self, inst, comp).kind {
            NodeKind::Composite { comp, .. } => comp.clone(),
            _ => return -2,
        };
        let res_code = match last {
            Res::Succeeded => 0,
            Res::Failed => 1,
            Res::Aborted => 2,
            Res::InProgress => 3,
        };
        match kind {
            CompositeKind::Selector | CompositeKind::Sequence | CompositeKind::Unconditional | CompositeKind::SimpleParallel => {
                // 0x140469340 [C]
                if prev == -1 {
                    return if n_children > 0 { 0 } else { -2 };
                }
                let go_on = match kind {
                    CompositeKind::Selector => res_code == 1,
                    CompositeKind::Sequence => res_code == 0,
                    CompositeKind::SimpleParallel => false,
                    _ => true,
                };
                if go_on && prev + 1 < n_children {
                    prev + 1
                } else {
                    -2
                }
            }
            CompositeKind::Random { weights, fail_on_all } => {
                // 0x140469390 / distribution pick 0x14118A3B0 [C]: Period < 0 -> weighted pick
                let slot = node!(self, inst, comp).mem;
                let mut m = self.take_mem(inst, slot);
                let r = if prev == -1 {
                    m.weights = weights.clone();
                    pick_weighted(&m.weights, &mut c.st.rng)
                } else if fail_on_all && res_code == 1 && prev < n_children {
                    if let Some(w) = m.weights.get_mut(prev as usize) {
                        *w = 0.0;
                    }
                    let p = pick_weighted(&m.weights, &mut c.st.rng);
                    if p == -1 {
                        -2
                    } else {
                        p
                    }
                } else {
                    -2
                };
                self.put_mem(inst, slot, m);
                if r == -1 {
                    -2
                } else {
                    r
                }
            }
        }
    }

    /// `UBTCompositeNode::GetNextChild` (override, search start).
    fn get_next_child(&mut self, c: &mut Ctx, inst: usize, comp: NodeIx, prev: i32, last: Res) -> i32 {
        let mut next = self.next_child_handler(c, inst, comp, prev, last);
        let slot = node!(self, inst, comp).mem;
        let ov = self.mem_ptr(inst, slot).override_child;
        if ov != -1 {
            next = ov;
            self.mem_ptr(inst, slot).override_child = -1;
        } else if let Some((si, se)) = self.search_start {
            if si == inst {
                let node = node!(self, inst, comp);
                if se > node.exec && se <= node.last_exec {
                    // the search start is inside this composite: go to the child containing it
                    let n = self.children_len(inst, comp);
                    let mut found = -2;
                    for i in 0..n {
                        let first = self.child_first_exec(inst, comp, i);
                        let next_first = self.child_first_exec(inst, comp, i + 1);
                        if se >= first && se < next_first {
                            found = i as i32;
                            break;
                        }
                    }
                    if found >= 0 {
                        next = found;
                        self.search_start = None;
                    }
                }
            }
        }
        next
    }

    /// `OnNodeActivation` of a composite: restart its memory, add its services, search start.
    fn composite_activate(&mut self, c: &mut Ctx, inst: usize, comp: NodeIx) {
        let slot = node!(self, inst, comp).mem;
        {
            let m = self.mem_ptr(inst, slot);
            m.cur_child = -1;
            m.override_child = -1;
        }
        let services = node!(self, inst, comp).services.clone();
        for s in services {
            self.add_update(AuxUpdate { inst, aux: s, mode: UpdateMode::Add });
            self.service_search_start(c, inst, s);
        }
    }

    fn service_search_start(&mut self, c: &mut Ctx, inst: usize, s: AuxIx) {
        let tree = self.stack[inst].tree.clone();
        let a = &tree.aux[s as usize];
        if let AuxKind::Service(svc) = &a.kind {
            let mut m = self.take_mem(inst, a.mem);
            svc.on_search_start(c, &mut m);
            if a.tick_on_search_start {
                svc.tick(c, &mut m, 0.0);
                // TickNode reschedules (0x143C43C10)
                m.next_tick = c.st.rng.deviate(a.interval, a.deviation);
            }
            self.put_mem(inst, a.mem, m);
        }
    }

    fn composite_deactivate(&mut self, _c: &mut Ctx, inst: usize, comp: NodeIx) {
        let services = node!(self, inst, comp).services.clone();
        for s in services {
            self.add_update(AuxUpdate { inst, aux: s, mode: UpdateMode::Remove });
        }
    }

    /// `OnChildActivation`: decorators' observers and activation, task services' search start,
    /// child composite activation.
    fn child_activate(&mut self, c: &mut Ctx, inst: usize, comp: NodeIx, child: usize) {
        let (decos, _) = self.child_decos(inst, comp, child);
        let tree = self.stack[inst].tree.clone();
        let pslot = node!(self, inst, comp).mem;
        let fresh = self.mem_ptr(inst, pslot).cur_child != child as i32;
        for d in decos {
            let a = &tree.aux[d as usize];
            match a.abort {
                AbortMode::LowerPriority => self.add_update(AuxUpdate { inst, aux: d, mode: UpdateMode::Remove }),
                AbortMode::SelfAbort | AbortMode::Both => self.add_update(AuxUpdate { inst, aux: d, mode: UpdateMode::Add }),
                AbortMode::None => {
                    // aux nodes that tick on their own (TimeLimit, Cooldown with no abort mode tick only with an abort mode in UE)
                    if let AuxKind::Decorator(dn) = &a.kind {
                        if dn.tick_intervals() {
                            self.add_update(AuxUpdate { inst, aux: d, mode: UpdateMode::Add });
                        }
                    }
                }
            }
            if let AuxKind::Decorator(dn) = &a.kind {
                let mut m = self.take_mem(inst, a.mem);
                dn.on_activation(c, &mut m, self.search_id, fresh);
                self.put_mem(inst, a.mem, m);
            }
        }
        let slot = node!(self, inst, comp).mem;
        self.mem_ptr(inst, slot).cur_child = child as i32;
        let cn = self.child_node(inst, comp, child);
        match &node!(self, inst, cn).kind {
            NodeKind::Composite { .. } => self.composite_activate(c, inst, cn),
            _ => {
                let services = node!(self, inst, cn).services.clone();
                for s in services {
                    self.service_search_start(c, inst, s);
                }
            }
        }
    }

    /// `NotifyDecoratorsOnFailedActivation`.
    fn child_failed_activation(&mut self, c: &mut Ctx, inst: usize, comp: NodeIx, child: usize, result: &mut Res) {
        let (decos, _) = self.child_decos(inst, comp, child);
        let tree = self.stack[inst].tree.clone();
        for d in decos {
            let a = &tree.aux[d as usize];
            if a.abort.observes_lower() {
                self.add_update(AuxUpdate { inst, aux: d, mode: UpdateMode::Add });
            }
            if let AuxKind::Decorator(dn) = &a.kind {
                let mut m = self.take_mem(inst, a.mem);
                dn.on_processed(c, &mut m, result);
                self.put_mem(inst, a.mem, m);
            }
        }
    }

    /// `OnChildDeactivation`: task services / child composite, then the decorators
    /// (`NotifyDecoratorsOnDeactivation`). `result` may be changed (ForceSuccess).
    fn child_deactivate(&mut self, c: &mut Ctx, inst: usize, comp: NodeIx, child: usize, result: &mut Res) {
        let cn = self.child_node(inst, comp, child);
        match &node!(self, inst, cn).kind {
            NodeKind::Composite { .. } => self.composite_deactivate(c, inst, cn),
            _ => {
                let services = node!(self, inst, cn).services.clone();
                for s in services {
                    self.add_update(AuxUpdate { inst, aux: s, mode: UpdateMode::Remove });
                }
            }
        }
        let (decos, _) = self.child_decos(inst, comp, child);
        let tree = self.stack[inst].tree.clone();
        let mut repeat = false;
        if *result == Res::Aborted {
            for &d in &decos {
                let a = &tree.aux[d as usize];
                if let AuxKind::Decorator(dn) = &a.kind {
                    let mut m = self.take_mem(inst, a.mem);
                    repeat |= dn.on_deactivation(c, &mut m, *result, self.search_id).repeat_child;
                    self.put_mem(inst, a.mem, m);
                }
                if a.abort == AbortMode::SelfAbort || a.abort == AbortMode::None {
                    self.add_update(AuxUpdate { inst, aux: d, mode: UpdateMode::Remove });
                }
            }
        } else {
            for &d in &decos {
                let a = &tree.aux[d as usize];
                if let AuxKind::Decorator(dn) = &a.kind {
                    let mut m = self.take_mem(inst, a.mem);
                    dn.on_processed(c, &mut m, result);
                    self.put_mem(inst, a.mem, m);
                }
            }
            for &d in &decos {
                let a = &tree.aux[d as usize];
                if let AuxKind::Decorator(dn) = &a.kind {
                    let mut m = self.take_mem(inst, a.mem);
                    repeat |= dn.on_deactivation(c, &mut m, *result, self.search_id).repeat_child;
                    self.put_mem(inst, a.mem, m);
                }
                match a.abort {
                    AbortMode::SelfAbort | AbortMode::None => self.add_update(AuxUpdate { inst, aux: d, mode: UpdateMode::Remove }),
                    AbortMode::LowerPriority | AbortMode::Both => self.add_update(AuxUpdate { inst, aux: d, mode: UpdateMode::Add }),
                }
            }
        }
        if repeat {
            // SetChildOverride (BTDecorator_Loop 0x143C66A00 -> parent vt+0x2C0)
            let slot = node!(self, inst, comp).mem;
            self.mem_ptr(inst, slot).override_child = child as i32;
        }
    }

    /// `FindChildToExecute`.
    fn find_child_to_execute(&mut self, c: &mut Ctx, inst: usize, comp: NodeIx, last: &mut Res) -> i32 {
        let n = self.children_len(inst, comp) as i32;
        if n == 0 {
            return -2;
        }
        let slot = node!(self, inst, comp).mem;
        let cur = self.mem_ptr(inst, slot).cur_child;
        let mut child = self.get_next_child(c, inst, comp, cur, *last);
        while child >= 0 && child < n {
            let (decos, ops) = self.child_decos(inst, comp, child as usize);
            if self.decorators_allow(c, inst, &decos, &ops) {
                self.child_activate(c, inst, comp, child as usize);
                return child;
            }
            *last = Res::Failed;
            if self.log_enabled {
                let cn = self.child_node(inst, comp, child as usize);
                let t = format!("skip {}:{} ({})", self.stack[inst].tree.name, node!(self, inst, cn).name, self.last_failed);
                self.trace(c.now(), t);
            }
            self.child_failed_activation(c, inst, comp, child as usize, last);
            child = self.get_next_child(c, inst, comp, child, *last);
        }
        -2
    }

    /// Is `child` of composite `comp` in instance `inst` on the execution path (`IsExecutingBranch`)?
    fn is_executing_branch(&self, inst: usize, comp: NodeIx, child: usize) -> bool {
        let Some(i) = self.stack.get(inst) else { return false };
        let Some(active) = i.active else { return false };
        let ae = i.tree.nodes[active as usize].exec;
        let first = self.child_first_exec(inst, comp, child);
        let next = self.child_first_exec(inst, comp, child + 1);
        ae >= first && ae < next
    }

    /// Deactivate from the active node up to composite `to` in instance `inst` (`DeactivateUpTo`).
    fn deactivate_up_to(&mut self, c: &mut Ctx, inst: usize, to: NodeIx, result: &mut Res) -> Option<i32> {
        let mut last_child = None;
        loop {
            let top = self.stack.len() - 1;
            match self.stack[top].active {
                None => {
                    if top > inst {
                        self.pop_instance(c, result);
                        continue;
                    }
                    return last_child;
                }
                Some(cur) => {
                    if top == inst && cur == to {
                        return last_child;
                    }
                    match node!(self, top, cur).parent {
                        Some(p) => {
                            let ci = node!(self, top, cur).child_index as usize;
                            self.child_deactivate(c, top, p, ci, result);
                            last_child = Some(ci as i32);
                            self.stack[top].active = Some(p);
                            self.stack[top].active_type = ActiveType::Composite;
                        }
                        None => {
                            if top > inst {
                                // leaving a subtree: deactivate its root, pop it; the parent
                                // instance's active node is the RunBehavior task
                                self.composite_deactivate(c, top, cur);
                                self.pop_instance(c, result);
                                continue;
                            }
                            return last_child;
                        }
                    }
                }
            }
        }
    }

    /// `FindCommonParent`: the lowest composite of instance `inst` that is an ancestor of both
    /// `comp` and the active execution path.
    fn common_parent(&self, inst: usize, comp: NodeIx) -> NodeIx {
        let tree = &self.stack[inst].tree;
        let mut anc = Vec::new();
        let mut cur = Some(comp);
        while let Some(n) = cur {
            anc.push(n);
            cur = tree.nodes[n as usize].parent;
        }
        let mut walk = self.stack[inst].active;
        while let Some(n) = walk {
            if anc.contains(&n) {
                return n;
            }
            walk = tree.nodes[n as usize].parent;
        }
        0
    }

    /// Remove the top instance (its aux nodes deactivated).
    fn pop_instance(&mut self, _c: &mut Ctx, _result: &mut Res) {
        let top = self.stack.len() - 1;
        let auxs = self.stack[top].aux_active.clone();
        for a in auxs {
            self.add_update(AuxUpdate { inst: top, aux: a, mode: UpdateMode::Remove });
        }
        // pending updates for the popped instance are dropped (DeactivateNodes)
        self.pending.retain(|u| !(u.inst == top && u.mode == UpdateMode::Add));
        // removes for the popped instance are applied immediately (their nodes cease relevance)
        let removes: Vec<AuxUpdate> = self.pending.iter().copied().filter(|u| u.inst == top).collect();
        self.pending.retain(|u| u.inst != top);
        self.popped_removes.extend(removes.into_iter().map(|u| (self.stack[top].tree.clone(), self.stack[top].known, u.aux)));
        self.stack.pop();
    }

    /// Process the pending request (`ProcessExecutionRequest`).
    fn process_request(&mut self, c: &mut Ctx) {
        let Some(req) = self.request.take() else { return };
        if req.inst >= self.stack.len() {
            return;
        }
        self.search_id = self.search_id.wrapping_add(1);
        // snapshot for rollback
        let snap_stack = self.stack.clone();
        // CopyInstanceMemoryToPersistent: only a restart (search_end set) can be rolled back
        let snap_known: Option<Vec<KnownInstance>> = if req.try_next_child { None } else { Some(self.known.clone()) };
        self.pending.clear();
        self.popped_removes.clear();
        self.search_start = None;

        let mut result = req.result;
        // deactivate up to the requested composite
        let top = self.stack.len() - 1;
        let at_target = top == req.inst && self.stack[top].active == Some(req.composite);
        if !at_target {
            if let Some(ci) = self.deactivate_up_to(c, req.inst, req.composite, &mut result) {
                let slot = node!(self, req.inst, req.composite).mem;
                self.mem_ptr(req.inst, slot).cur_child = ci;
            }
        }
        let mut inst = self.stack.len() - 1;
        if self.stack[inst].active.is_none() {
            // activate the root node
            self.stack[inst].active = Some(0);
            self.composite_activate(c, inst, 0);
        }
        let mut search_end = None;
        if !req.try_next_child {
            if let Some(ss) = req.search_start {
                let deact = if ss.1 > 0 { (ss.0, ss.1 - 1) } else { ss };
                self.unregister_aux_up_to(deact);
                let slot = node!(self, req.inst, req.composite).mem;
                let m = self.mem_ptr(req.inst, slot);
                m.cur_child = -1;
                m.override_child = -1;
                self.search_start = Some(ss);
                search_end = req.search_end;
            }
        }
        // search
        let mut test = Some(req.composite);
        let mut next_task: Option<NodeIx> = None;
        let mut guard = 0;
        while let Some(tn) = test {
            guard += 1;
            if guard > 10_000 {
                break;
            }
            inst = self.stack.len() - 1;
            let child = self.find_child_to_execute(c, inst, tn, &mut result);
            if child == -2 {
                // return to parent
                let parent = node!(self, inst, tn).parent;
                match parent {
                    Some(p) => {
                        let ci = node!(self, inst, tn).child_index as usize;
                        self.child_deactivate(c, inst, p, ci, &mut result);
                        self.stack[inst].active = Some(p);
                        test = Some(p);
                    }
                    None => {
                        if inst > 0 {
                            // leave the subtree: the RunBehavior task in the parent finishes with `result`
                            self.composite_deactivate(c, inst, tn);
                            let mut r = result;
                            self.pop_instance(c, &mut r);
                            let pi = self.stack.len() - 1;
                            let task = self.stack[pi].active.unwrap_or(0);
                            match node!(self, pi, task).parent {
                                Some(p) => {
                                    let ci = node!(self, pi, task).child_index as usize;
                                    self.child_deactivate(c, pi, p, ci, &mut result);
                                    self.stack[pi].active = Some(p);
                                    self.stack[pi].active_type = ActiveType::Composite;
                                    test = Some(p);
                                }
                                None => test = None,
                            }
                        } else {
                            // root finished: OnTreeFinished restarts it (looped execution)
                            self.composite_deactivate(c, inst, tn);
                            test = None;
                        }
                    }
                }
            } else {
                let cn = self.child_node(inst, tn, child as usize);
                self.stack[inst].active = Some(tn);
                match &node!(self, inst, cn).kind {
                    NodeKind::Composite { .. } => {
                        self.stack[inst].active = Some(cn);
                        test = Some(cn);
                    }
                    _ => {
                        next_task = Some(cn);
                        test = None;
                    }
                }
            }
        }
        self.search_start = None;

        let valid = match (next_task, &search_end) {
            (Some(t), Some(end)) => takes_priority((self.stack.len() - 1, node!(self, self.stack.len() - 1, t).exec), *end),
            (None, Some(_)) => false,
            _ => true,
        };
        if !valid {
            if self.log_enabled {
                let found = next_task.map(|t| (self.stack.len() - 1, node!(self, self.stack.len() - 1, t).exec, node!(self, self.stack.len() - 1, t).name.clone()));
                self.trace(c.now(), format!("rollback: found {found:?}, search end {search_end:?}"));
            }
            // RollbackSearchChanges
            self.stack = snap_stack;
            if let Some(k) = snap_known {
                // CopyInstanceMemoryFromPersistent (instances created by the search stay known)
                let n = k.len();
                for (dst, src) in self.known.iter_mut().zip(k.into_iter()) {
                    *dst = src;
                }
                let _ = n;
            }
            self.pending.clear();
            self.popped_removes.clear();
            return;
        }
        // abort the running task (it lives in the snapshot's stack)
        let running = snap_stack.iter().enumerate().rev().find_map(|(ii, i)| {
            if matches!(i.active_type, ActiveType::ActiveTask) {
                i.active.map(|n| (ii, i.tree.clone(), i.known, n))
            } else {
                None
            }
        });
        let mut latent = false;
        if let Some((ii, tree, known, n)) = running {
            if let NodeKind::Task(t) = &tree.nodes[n as usize].kind {
                let slot = tree.nodes[n as usize].mem;
                let mut m = std::mem::take(&mut self.known[known].mem[slot as usize]);
                let r = t.abort(c, &mut m);
                if r == Res::InProgress {
                    latent = true;
                    self.aborting = Some((tree.clone(), known, n, ii));
                } else {
                    t.on_finished(c, &mut m, Res::Aborted);
                }
                self.known[known].mem[slot as usize] = m;
                let label = format!("abort {}:{}", tree.name, tree.nodes[n as usize].name);
                self.trace(c.now(), label);
            }
        }
        // cease relevance of aux nodes of popped instances
        let popped = std::mem::take(&mut self.popped_removes);
        for (tree, known, a) in popped {
            let ax = &tree.aux[a as usize];
            let mut m = std::mem::take(&mut self.known[known].mem[ax.mem as usize]);
            match &ax.kind {
                AuxKind::Decorator(d) => d.on_cease_relevant(c, &mut m),
                AuxKind::Service(s) => s.on_cease_relevant(c, &mut m),
            }
            self.known[known].mem[ax.mem as usize] = m;
        }
        match next_task {
            Some(t) => {
                let inst = self.stack.len() - 1;
                if latent {
                    self.pending_task = Some((inst, t));
                } else {
                    self.execute_task(c, inst, t);
                }
            }
            None => {
                // OnTreeFinished: restart the root (deferred request)
                self.apply_updates(c);
                while self.stack.len() > 1 {
                    self.stack.pop();
                }
                self.stack[0].active = None;
                self.stack[0].active_type = ActiveType::Composite;
                self.unregister_aux_up_to((0, 0));
                self.apply_updates(c);
                self.request = Some(Request { inst: 0, composite: 0, child: -1, result: Res::InProgress, try_next_child: true, search_start: None, search_end: None });
                self.trace(c.now(), format!("{} finished, restart", self.stack[0].tree.name));
            }
        }
    }

    /// `ExecuteTask` (and RunBehavior's push).
    fn execute_task(&mut self, c: &mut Ctx, inst: usize, n: NodeIx) {
        // task services become relevant
        let services = node!(self, inst, n).services.clone();
        for s in services {
            self.add_update(AuxUpdate { inst, aux: s, mode: UpdateMode::Add });
        }
        self.apply_updates(c);
        self.stack[inst].active = Some(n);
        self.stack[inst].active_type = ActiveType::ActiveTask;
        let tree = self.stack[inst].tree.clone();
        let node = &tree.nodes[n as usize];
        match &node.kind {
            NodeKind::RunBehavior { asset, tag } => {
                let mut name = asset.clone();
                if let Some(tag) = tag {
                    if let Some((_, t)) = self.dynamic.iter().find(|(tg, _)| tg == tag) {
                        name = Some(t.clone());
                    }
                }
                let sub = name.and_then(|nm| self.lib.trees.get(&nm).cloned());
                let Some(sub) = sub else {
                    self.trace(c.now(), format!("RunBehavior {}: no tree", node.name));
                    self.finish_task(c, inst, n, Res::Failed);
                    return;
                };
                if sub.nodes.is_empty() {
                    self.finish_task(c, inst, n, Res::Failed);
                    return;
                }
                // instance id path: execution indices of the RunBehavior tasks
                let mut path: Vec<u16> = Vec::new();
                for (ii, i) in self.stack.iter().enumerate() {
                    if ii <= inst {
                        if let Some(a) = i.active {
                            path.push(i.tree.nodes[a as usize].exec);
                        }
                    }
                }
                let known = self.known_instance(&sub.name, path, sub.mem_slots);
                self.stack.push(Instance { tree: sub.clone(), known, active: None, active_type: ActiveType::Composite, aux_active: Vec::new() });
                let ni = self.stack.len() - 1;
                // root decorators (PushInstance)
                let rd = sub.root_decorators.clone();
                let ro = sub.root_ops.clone();
                if !self.decorators_allow(c, ni, &rd, &ro) {
                    self.stack.pop();
                    self.finish_task(c, inst, n, Res::Failed);
                    return;
                }
                self.trace(c.now(), format!("push {}", sub.name));
                self.request = Some(Request { inst: ni, composite: 0, child: -1, result: Res::InProgress, try_next_child: true, search_start: None, search_end: None });
            }
            NodeKind::Task(t) => {
                let mut m = self.take_mem(inst, node.mem);
                let r = t.execute(c, &mut m);
                self.put_mem(inst, node.mem, m);
                if self.log_enabled {
                    let d = t.describe();
                    self.trace(c.now(), format!("exec {}:{} {} -> {:?}", tree.name, node.name, d, r));
                }
                if r != Res::InProgress {
                    self.finish_task(c, inst, n, r);
                }
            }
            NodeKind::Composite { .. } => {}
        }
    }

    /// `OnTaskFinished` for the active task: request execution from its parent.
    fn finish_task(&mut self, c: &mut Ctx, inst: usize, n: NodeIx, r: Res) {
        let tree = self.stack[inst].tree.clone();
        let node = &tree.nodes[n as usize];
        if let NodeKind::Task(t) = &node.kind {
            let mut m = self.take_mem(inst, node.mem);
            t.on_finished(c, &mut m, r);
            self.put_mem(inst, node.mem, m);
        }
        if self.stack[inst].active == Some(n) {
            self.stack[inst].active_type = ActiveType::InactiveTask;
            if let Some(p) = node.parent {
                self.request = Some(Request { inst, composite: p, child: node.child_index as i32, result: r, try_next_child: true, search_start: None, search_end: None });
            }
        }
    }

    /// `RequestExecution(UBTDecorator*)` from an observing decorator.
    fn request_from_decorator(&mut self, c: &mut Ctx, inst: usize, aux: AuxIx) {
        let tree = self.stack[inst].tree.clone();
        let a = &tree.aux[aux as usize];
        let Some(comp) = a.owner else { return };
        let child = a.child_index as usize;
        let executing = self.is_executing_branch(inst, comp, child);
        let cont = match a.abort {
            AbortMode::SelfAbort => Res::Failed,
            AbortMode::Both => {
                if executing {
                    Res::Failed
                } else {
                    Res::Aborted
                }
            }
            _ => Res::Aborted,
        };
        let req = if cont == Res::Aborted {
            // switch to higher priority (search from the requester's child)
            let start = (inst, self.child_first_exec(inst, comp, child));
            // 0x143C42290 [C]: a restart is dropped when the requested child's decorators do not
            // allow it now (unless its branch executes), or when a branch on the way from the
            // requester up to the common parent with the active node does not
            if !executing {
                let (decos, ops) = self.child_decos(inst, comp, child);
                if !self.decorators_allow(c, inst, &decos, &ops) {
                    return;
                }
            }
            // the search runs from the common parent of the requester and the active path
            let common = self.common_parent(inst, comp);
            let mut n = comp;
            while n != common {
                let Some(p) = node!(self, inst, n).parent else { break };
                let ci = node!(self, inst, n).child_index as usize;
                let (decos, ops) = self.child_decos(inst, p, ci);
                if !self.decorators_allow(c, inst, &decos, &ops) {
                    return;
                }
                n = p;
            }
            let search_end = (inst, self.child_first_exec(inst, comp, child + 1));
            Request { inst, composite: common, child: child as i32, result: Res::Aborted, try_next_child: false, search_start: Some(start), search_end: Some(search_end) }
        } else {
            if !executing {
                return;
            }
            Request { inst, composite: comp, child: child as i32, result: Res::Failed, try_next_child: true, search_start: None, search_end: None }
        };
        // a pending continuation (task finished / subtree pushed) is never replaced: the observer
        // asks again on its next tick
        // ... unless the new one is a higher-priority restart (UE RequestExecution keeps the most
        // important request; C3: without this a tree whose loops re-push a failing subtree every
        // tick never let a LowerPriority observer above it in, e.g. CanBattle over the field AI)
        if let Some(old) = self.request.as_ref().filter(|r| r.try_next_child) {
            let old_ix = (old.inst, node!(self, old.inst, old.composite).exec);
            let higher = req.search_start.map(|s| takes_priority(s, old_ix) && s.0 <= old.inst).unwrap_or(false);
            if !higher || self.stack.len() - 1 != old.inst || old.inst == req.inst {
                return;
            }
        }
        // a request already pending with higher priority wins (UE keeps the most important one);
        // 0x143C42290: a restart merged into it moves its SearchEnd later, a Self abort clears it
        if let Some(old) = &mut self.request {
            let old_ix = (old.inst, node!(self, old.inst, old.composite).exec);
            let new_ix = (req.inst, node!(self, req.inst, req.composite).exec);
            if takes_priority(old_ix, new_ix) {
                if let Some(oe) = old.search_end {
                    match req.search_end {
                        Some(ne) if takes_priority(oe, ne) => old.search_end = Some(ne),
                        Some(_) => {}
                        None => old.search_end = None,
                    }
                }
                return;
            }
        }
        self.request = Some(req);
    }

    /// Run the observers of changed blackboard keys.
    fn process_bb_changes(&mut self, c: &mut Ctx) {
        let changed = c.bb.take_changes();
        if changed.is_empty() {
            return;
        }
        let names: Vec<String> = changed.iter().map(|k| c.bb.key_name(*k).to_string()).collect();
        for ii in 0..self.stack.len() {
            let tree = self.stack[ii].tree.clone();
            let list = self.stack[ii].aux_active.clone();
            for a in list {
                let ax = &tree.aux[a as usize];
                if let AuxKind::Decorator(d) = &ax.kind {
                    if let Some(k) = d.observed_key() {
                        if names.iter().any(|n| n == k) && ax.abort != AbortMode::None {
                            let executing = ax.owner.map(|o| self.is_executing_branch(ii, o, ax.child_index as usize)).unwrap_or(false);
                            let mut m = self.take_mem(ii, ax.mem);
                            let req = d.on_key_change(c, &mut m, executing, ax.inverse);
                            self.put_mem(ii, ax.mem, m);
                            if req {
                                self.request_from_decorator(c, ii, a);
                            }
                        }
                    }
                }
            }
        }
    }

    /// Tick the active aux nodes (services with intervals, ticking decorators).
    fn tick_aux(&mut self, c: &mut Ctx, dt: f32) {
        for ii in 0..self.stack.len() {
            let tree = self.stack[ii].tree.clone();
            let list = self.stack[ii].aux_active.clone();
            for a in list {
                if ii >= self.stack.len() || !self.stack[ii].aux_active.contains(&a) {
                    continue;
                }
                let ax = &tree.aux[a as usize];
                let mut m = self.take_mem(ii, ax.mem);
                let mut request = false;
                match &ax.kind {
                    AuxKind::Service(s) => {
                        // bTickIntervals: NextTickRemainingTime (0x143C493E0 wrapper)
                        m.next_tick -= dt;
                        m.accum += dt;
                        if m.next_tick <= 0.0 {
                            let acc = m.accum;
                            m.accum = 0.0;
                            s.tick(c, &mut m, acc);
                            m.next_tick = c.st.rng.deviate(ax.interval, ax.deviation);
                        }
                    }
                    AuxKind::Decorator(d) => {
                        if d.tick_intervals() {
                            m.next_tick -= dt;
                            m.accum += dt;
                            if m.next_tick <= 0.0 && m.flag & 0x8000_0000 == 0 {
                                m.flag |= 0x8000_0000;
                                let executing = ax.owner.map(|o| self.is_executing_branch(ii, o, ax.child_index as usize)).unwrap_or(false);
                                let acc = m.accum;
                                request = d.tick(c, &mut m, acc, executing);
                                m.accum = 0.0;
                            }
                        } else if d.ticks() && ax.abort != AbortMode::None {
                            let executing = ax.owner.map(|o| self.is_executing_branch(ii, o, ax.child_index as usize)).unwrap_or(false);
                            request = d.tick(c, &mut m, dt, executing);
                        } else if ax.cond_tick && ax.abort != AbortMode::None {
                            let executing = ax.owner.map(|o| self.is_executing_branch(ii, o, ax.child_index as usize)).unwrap_or(false);
                            let pass = d.calc(c, &mut m) != ax.inverse;
                            request = pass != executing;
                        }
                    }
                }
                self.put_mem(ii, ax.mem, m);
                if request {
                    self.request_from_decorator(c, ii, a);
                }
            }
        }
        self.process_bb_changes(c);
    }

    fn tick_active_task(&mut self, c: &mut Ctx, dt: f32) {
        // a latently aborting task first
        if let Some((tree, known, n, _ii)) = self.aborting.clone() {
            if let NodeKind::Task(t) = &tree.nodes[n as usize].kind {
                let slot = tree.nodes[n as usize].mem;
                let mut m = std::mem::take(&mut self.known[known].mem[slot as usize]);
                let r = t.tick(c, &mut m, dt);
                if r.is_some() {
                    t.on_finished(c, &mut m, Res::Aborted);
                }
                self.known[known].mem[slot as usize] = m;
                if r.is_some() {
                    self.aborting = None;
                    if let Some((inst, t)) = self.pending_task.take() {
                        if inst < self.stack.len() {
                            self.execute_task(c, inst, t);
                        }
                    }
                }
            }
            return;
        }
        let top = self.stack.len().saturating_sub(1);
        let Some(inst) = self.stack.get(top) else { return };
        if inst.active_type != ActiveType::ActiveTask {
            return;
        }
        let Some(n) = inst.active else { return };
        let tree = inst.tree.clone();
        if let NodeKind::Task(t) = &tree.nodes[n as usize].kind {
            let slot = tree.nodes[n as usize].mem;
            let mut m = self.take_mem(top, slot);
            let r = t.tick(c, &mut m, dt);
            self.put_mem(top, slot, m);
            if let Some(r) = r {
                if self.log_enabled {
                    self.trace(c.now(), format!("done {}:{} -> {:?}", tree.name, tree.nodes[n as usize].name, r));
                }
                self.finish_task(c, top, n, r);
            }
        }
    }

    /// One `TickComponent`.
    pub fn tick(&mut self, c: &mut Ctx, dt: f32) {
        if self.stack.is_empty() {
            return;
        }
        self.tick_aux(c, dt);
        if self.request.is_some() && self.aborting.is_none() {
            self.process_request(c);
            self.process_bb_changes(c);
        }
        self.tick_active_task(c, dt);
        self.process_bb_changes(c);
        if self.request.is_some() && self.aborting.is_none() {
            self.process_request(c);
            self.process_bb_changes(c);
        }
    }

    /// For tests / the reachability report: the compiled library.
    pub fn library(&self) -> &Library {
        &self.lib
    }
}

/// The weighted pick of 0x14118A3B0 (Period < 0): `r = frand * sum`, walk the weights.
fn pick_weighted(w: &[f32], rng: &mut crate::rng::Rng) -> i32 {
    let sum: f32 = w.iter().sum();
    if sum == 0.0 {
        return -1;
    }
    let mut r = rng.frand() * sum;
    for (i, x) in w.iter().enumerate() {
        if r <= *x {
            return i as i32;
        }
        r -= x;
    }
    w.len() as i32 - 1
}
