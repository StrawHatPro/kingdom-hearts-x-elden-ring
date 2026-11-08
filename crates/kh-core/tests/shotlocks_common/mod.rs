//! (SHOT-A) The shared bench of the shotlock trace tests (`tests/shotlock_<family>.rs`: `mod shotlocks_common;`): the
//! level-99 loadout, training dummies, a Keyblade in slot 0, the aim mode driven through the pad (RB held, A fires) as
//! tests/magic.rs does, and a trace of every step for the assertions. Every helper is used by `the_bench_works` below
//! (it runs in each test binary that includes the module, so no helper is dead code there).

use kh_core::host::HitRecord;
use kh_core::magic::View;
use kh_core::status::{KeybladeId, LoadoutValue};
use kh_core::test_world::TestWorld;
use kh_core::{AnimId, Input, Loadout, Output, PadButton, PadButtons, PadState, Pose, RecordingHost, Sora, StateId, Status, TargetFlags, TargetId, TargetInfo, Tuning, Vec3, SORA_PARAMS};

pub const DT: f32 = 1.0 / 60.0;

fn cell(json: &serde_json::Value) -> Option<LoadoutValue> {
    match json {
        serde_json::Value::Number(n) => n.as_f64().map(LoadoutValue::Number),
        serde_json::Value::String(s) => Some(LoadoutValue::Text(s.clone())),
        serde_json::Value::Array(items) => items.iter().map(|i| i.as_str().map(str::to_string)).collect::<Option<Vec<_>>>().map(LoadoutValue::List),
        serde_json::Value::Object(map) => map.iter().map(|(k, v)| v.as_f64().map(|n| (k.clone(), n))).collect::<Option<Vec<_>>>().map(LoadoutValue::Map),
        _ => None,
    }
}

/// The level-99 loadout with no ability equipped, holding the Keyblade `key` ("WEP_KEYBLADE_SO_01") in slot 0.
pub fn level99_with(key: &str) -> Status {
    let path = format!("{}/../../sheets/port/sora-loadout-99.json", env!("CARGO_MANIFEST_DIR"));
    let json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))).unwrap();
    let mut loadout = Loadout::new("sora-loadout-99");
    for row in json["rows"].as_array().unwrap() {
        let key = row[0].as_str().unwrap();
        let value = if key == "abilities.equipped" { LoadoutValue::List(Vec::new()) } else { cell(&row[1]).unwrap() };
        loadout.set(key, &value).unwrap_or_else(|e| panic!("{e}"));
    }
    loadout.resolve().unwrap();
    let mut status = Status::from_loadout(&loadout);
    let id = KeybladeId::from_key(key).unwrap_or_else(|| panic!("no Keyblade {key}"));
    status.set_keyblade_slot(0, Some(id)).unwrap();
    let mut k = *status.keyblades();
    k.set_current(0).unwrap();
    status.set_keyblades(k);
    assert_eq!(status.current_keyblade(), Some(id));
    status
}

/// A training dummy `id` at (x, y) on the floor (the lock-on accepts it: ALIVE | LOCKABLE | ENEMY).
pub fn dummy(id: u32, x: f32, y: f32) -> TargetInfo {
    TargetInfo { id: TargetId(id), position: Vec3::new(x, y, 0.0), center: Vec3::new(x, y, 90.0), velocity: Vec3::ZERO, yaw: 180.0, radius: 40.0, half_height: 90.0, flags: TargetFlags(TargetFlags::ALIVE | TargetFlags::HITTABLE | TargetFlags::LOCKABLE | TargetFlags::ENEMY), hp: 1.0e6, hp_max: 1.0e6 }
}

/// One step as the trace records it.
#[derive(Clone, Debug)]
pub struct TraceStep {
    pub t: f32,
    pub state: StateId,
    pub sub: u8,
    pub clip: AnimId,
    pub position: Vec3,
    pub projectiles: usize,
    pub hits: usize,
}

pub struct Bench {
    pub sora: Sora,
    pub host: RecordingHost,
    pub world: TestWorld,
    held: PadButtons,
    /// A view from behind Sora looking along +X (the reticle test's camera).
    pub view: Option<View>,
    pub time: f32,
}

impl Bench {
    pub fn new(status: Status, dummies: &[TargetInfo]) -> Bench {
        let origin = Vec3::new(0.0, 0.0, Sora::origin_height_on_floor(&SORA_PARAMS));
        let sora = Sora::new(SORA_PARAMS, Pose { position: origin, yaw: 0.0 }).with_tuning(Tuning::GAME_EXACT).with_status(status);
        let mut host = RecordingHost::new();
        host.targets.extend_from_slice(dummies);
        let mut bench = Bench { sora, host, world: TestWorld::floor(), held: PadButtons::NONE, view: Some(View::from_camera(Vec3::new(-300.0, 0.0, 150.0), 0.0, 0.0, 90.0, 1280, 720)), time: 0.0 };
        bench.run(5);
        bench
    }

    /// One step with these buttons down.
    pub fn step(&mut self, held: &[PadButton]) -> Output {
        let now = held.iter().fold(PadButtons::NONE, |all, b| all.with(*b));
        let pad = PadState::from_held(now, self.held);
        self.held = now;
        let mut input = Input::default().with_pad(pad);
        input.view = self.view;
        let (sora, world) = (&mut self.sora, &self.world);
        self.time += DT;
        self.host.step(world, |h| sora.step_hosted(DT, &input, h))
    }

    pub fn run(&mut self, steps: usize) -> Vec<Output> {
        (0..steps).map(|_| self.step(&[])).collect()
    }

    pub fn hold(&mut self, held: &[PadButton], steps: usize) -> Vec<Output> {
        (0..steps).map(|_| self.step(held)).collect()
    }

    pub fn hits(&self) -> Vec<HitRecord> {
        self.host.hits().into_iter().map(|h| h.0).collect()
    }

    /// RB held until `locks` locks (or MAX when `locks` is 0); panics after `max_steps`. Returns the aim's kind.
    pub fn aim_until(&mut self, locks: i32, max_steps: usize) -> u8 {
        for _ in 0..max_steps {
            self.step(&[PadButton::RightShoulder]);
            let aim = self.sora.aim();
            if (locks > 0 && aim.total >= locks) || (locks == 0 && aim.is_max) {
                return aim.kind;
            }
        }
        panic!("the aim never reached {locks} locks (now {:?})", self.sora.aim());
    }

    /// A pressed with RB held: the shotlock fires; then steps until the state is ShootFlow (at most 3).
    pub fn fire(&mut self) {
        self.step(&[PadButton::RightShoulder, PadButton::ATTACK]);
        for _ in 0..3 {
            if self.step(&[]).state == StateId::Shootflow {
                return;
            }
        }
        panic!("the shotlock did not start (aim {:?})", self.sora.aim());
    }

    /// Steps (no buttons) while the state is ShootFlow, at most `max_steps`; the trace of every step.
    pub fn trace_shoot_flow(&mut self, max_steps: usize) -> Vec<TraceStep> {
        let mut out = Vec::new();
        for _ in 0..max_steps {
            let o = self.step(&[]);
            out.push(TraceStep { t: self.time, state: o.state, sub: o.sub_step, clip: o.anim.name, position: o.position, projectiles: self.sora.magic().projectile_views().count(), hits: self.host.hits().len() });
            if o.state != StateId::Shootflow {
                return out;
            }
        }
        panic!("ShootFlow did not end within {max_steps} steps");
    }

    /// After a shotlock: the aim mode is closed, every type is at full speed and Sora is back in a normal state.
    pub fn assert_back_to_normal(&mut self) {
        self.run(120);
        let aim = self.sora.aim();
        assert_eq!(aim.mode, 0, "aim mode closed");
        assert_eq!((self.sora.time_dilation(1), self.sora.time_dilation(2)), (1.0, 1.0), "full speed");
        let state = self.step(&[]).state;
        assert!(matches!(state, StateId::Idle | StateId::Run | StateId::Jump), "back to normal: {state:?}");
        assert!(!self.sora.status().damage_invalid, "the invincibility ended");
    }
}

pub fn seconds(steps: usize) -> f32 {
    steps as f32 * DT
}

/// The bench itself, with the Kingdom Key (King of Hearts): lock-on -> fire -> finish -> back to normal.
#[test]
fn the_bench_works() {
    let mut b = Bench::new(level99_with("WEP_KEYBLADE_SO_00"), &[dummy(1, 800.0, 0.0)]);
    b.world = TestWorld::floor();
    b.hold(&[], 1);
    assert_eq!(b.aim_until(3, 200), 1);
    b.fire();
    let trace = b.trace_shoot_flow(600);
    let last = trace.last().unwrap();
    assert_ne!(last.state, StateId::Shootflow);
    assert!(last.t > 3.0 && seconds(trace.len()) > 3.0, "about 4 s");
    assert!(trace.iter().any(|s| s.clip == AnimId::M1473 && matches!(s.sub, 3 | 4)), "the charge clip");
    assert!(trace.iter().any(|s| s.projectiles > 0 && s.position.z > 300.0), "lifted, firing");
    b.run(240);
    assert!(trace.last().unwrap().hits <= b.hits().len() && !b.hits().is_empty());
    b.assert_back_to_normal();
    assert!(b.time > 0.0 && b.view.is_some());
}
