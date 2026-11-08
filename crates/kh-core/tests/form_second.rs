//! (weapons F-SECOND) Second Form S (Starlight, `form.L2`) and Second Form D (Dead of Night, `form.L5`) from the offer to
//! the end, through the pad path: the transformation, a combo in the form, the finish levels, the level-3 finisher
//! (Ancient Light 0xAE in Second Form S: STYLEFINISH slot 155 0x1413E2810; Last Arcanum 0xAD in Second Form D), the form's
//! end; and the XS Ancient Light offer beside each Keyblade's own Second Form (AwardAtMax 0x1413F0ED0's table).
//!
//! `KH_F_SECOND_TRACES=<dir>` writes the step traces (`l2_session.trace`, `l5_session.trace`) there.

use kh_core::enums::ETresAbilityKind as A;
use kh_core::host::{BodyReaction, TargetBody};
use kh_core::status::LoadoutValue;
use kh_core::test_world::TestWorld;
use kh_core::{Input, Loadout, Output, PadButton, PadButtons, PadState, Pose, RecordingHost, Sora, StateId, Status, TargetFlags, TargetId, TargetInfo, Tuning, Vec3, SORA_PARAMS};
use std::fmt::Write as _;

const DT: f32 = 1.0 / 60.0;
const STARLIGHT: &str = "WEP_KEYBLADE_SO_17";
const DEAD_OF_NIGHT: &str = "WEP_KEYBLADE_SO_18";
const KINGDOM_KEY: &str = "WEP_KEYBLADE_SO_00";

fn cell(json: &serde_json::Value) -> Option<LoadoutValue> {
    match json {
        serde_json::Value::Number(n) => n.as_f64().map(LoadoutValue::Number),
        serde_json::Value::String(s) => Some(LoadoutValue::Text(s.clone())),
        serde_json::Value::Array(items) => items.iter().map(|i| i.as_str().map(str::to_string)).collect::<Option<Vec<_>>>().map(LoadoutValue::List),
        serde_json::Value::Object(map) => map.iter().map(|(k, v)| v.as_f64().map(|n| (k.clone(), n))).collect::<Option<Vec<_>>>().map(LoadoutValue::Map),
        _ => None,
    }
}

/// The level-99 loadout with no equipped abilities but `extra`, holding the Keyblade `key`.
fn holding(key: &str, extra: &[A]) -> Status {
    let path = format!("{}/../../sheets/port/sora-loadout-99.json", env!("CARGO_MANIFEST_DIR"));
    let json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))).unwrap();
    let mut loadout = Loadout::new("sora-loadout-99");
    for row in json["rows"].as_array().unwrap() {
        let k = row[0].as_str().unwrap();
        let value = if k == "abilities.equipped" { LoadoutValue::List(Vec::new()) } else { cell(&row[1]).unwrap() };
        loadout.set(k, &value).unwrap_or_else(|e| panic!("{e}"));
    }
    loadout.resolve().unwrap();
    let mut s = Status::from_loadout(&loadout);
    s.weapon = key.to_string();
    for &kind in extra {
        let owned = s.abilities.owned(kind).max(1);
        s.abilities.set(kind, owned, 1);
    }
    s
}

fn dummy(id: u32, x: f32, y: f32) -> TargetInfo {
    let at = Vec3::new(x, y, 0.0);
    TargetInfo { id: TargetId(id), position: at, center: at + Vec3::Z * 90.0, velocity: Vec3::ZERO, yaw: 180.0, radius: 40.0, half_height: 90.0, flags: TargetFlags(TargetFlags::ALIVE | TargetFlags::HITTABLE | TargetFlags::LOCKABLE | TargetFlags::ENEMY), hp: 1.0e6, hp_max: 1.0e6 }
}

fn pad(held: &[PadButton]) -> PadState {
    let set = |list: &[PadButton]| list.iter().fold(PadButtons::NONE, |all, b| all.with(*b));
    PadState::from_held(set(held), PadButtons::NONE)
}

struct Bench {
    sora: Sora,
    host: RecordingHost,
    world: TestWorld,
    trace: String,
    steps: usize,
    /// The camera (the lock-on keeps a target only on screen: as tests/xsi_abilities.rs).
    view: Option<kh_core::magic::View>,
}

impl Bench {
    fn new(status: Status, dummies: &[TargetInfo]) -> Bench {
        let origin = Vec3::new(0.0, 0.0, Sora::origin_height_on_floor(&SORA_PARAMS));
        let sora = Sora::new(SORA_PARAMS, Pose { position: origin, yaw: 0.0 }).with_tuning(Tuning::GAME_EXACT).with_status(status);
        let mut host = RecordingHost::new();
        host.targets.extend_from_slice(dummies);
        let mut b = Bench { sora, host, world: TestWorld::floor(), trace: String::new(), steps: 0, view: None };
        for _ in 0..5 {
            b.step(&[]);
        }
        b
    }

    /// One step, traced: state / sub-step, height, clip, the form (command, finish level / command, wear, world slow), the
    /// queue's kinds and slot B, the finish state's view, the step's hits and events.
    fn step(&mut self, held: &[PadButton]) -> Output {
        let hits_before = self.host.hits().len();
        let mut input = Input::default().with_pad(pad(held));
        input.view = self.view;
        let (sora, world) = (&mut self.sora, &self.world);
        let out = self.host.step(world, |h| sora.step_hosted(DT, &input, h));
        self.steps += 1;
        let c = self.sora.combat();
        let f = &c.form;
        let q = &c.situation;
        let _ = write!(
            self.trace,
            "{} {:?}/{} z={:.1} a={} | form {:#x} lv {} fin {:#x} wear {} slow {} t {:.2} | q {:?} b={:?} x={:?}",
            self.steps,
            out.state,
            out.sub_step,
            out.position.z,
            out.anim.name.name(),
            f.cmd,
            f.finish_level,
            f.finish_cmd,
            f.wear_form,
            f.world_slow,
            f.time_left,
            q.list.iter().map(|e| e.kind).collect::<Vec<_>>(),
            q.slot_b.map(|e| e.kind),
            q.extra,
        );
        if let Some(fv) = self.sora.finish() {
            let _ = write!(self.trace, " | finish {:#x} sub {} count {} pending {:#x}", fv.cmd, fv.sub, fv.count, fv.pending);
        }
        for (h, _) in self.host.hits().iter().skip(hits_before) {
            let _ = write!(self.trace, " | hit {} {} {:.1}", h.target.0, h.attack_name, h.damage);
        }
        for e in &out.events {
            let _ = write!(self.trace, " | ev {:?}", e.kind);
        }
        self.trace.push('\n');
        out
    }

    fn tap(&mut self, button: PadButton) -> Output {
        self.step(&[button])
    }

    fn idle(&mut self, steps: usize) {
        for _ in 0..steps {
            self.step(&[]);
        }
    }

    fn mark(&mut self, what: &str) {
        let _ = writeln!(self.trace, "== {what}");
    }

    fn kinds(&self) -> Vec<i32> {
        self.sora.combat().situation.list.iter().map(|e| e.kind).collect()
    }

    /// The formchange / finish / XS kinds the queue holds anywhere (the list and the slots: `Queue::has`).
    fn held(&self) -> Vec<i32> {
        [0x80, 0x94, 0x95, 0x98, 0xAB, 0xAC, 0xAD, 0xAE].into_iter().filter(|&k| self.sora.combat().situation.has(k)).collect()
    }

    fn lock(&mut self, id: u32) {
        let lock = &mut self.sora.combat_mut().lockon;
        lock.locked = true;
        lock.current = TargetId(id);
    }

    fn save(&self, name: &str) {
        if let Some(dir) = std::env::var_os("KH_F_SECOND_TRACES") {
            let dir = std::path::PathBuf::from(dir);
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join(name), &self.trace).unwrap();
        }
    }

    /// The offer, Triangle, the transformation to its end.
    fn enter(&mut self, cmd: i32) {
        self.mark("offer");
        self.sora.debug_add_style(100.0);
        self.idle(15);
        assert_eq!(self.kinds(), [cmd], "the gauge offers the held Keyblade's Second Form");
        self.mark("transform");
        self.tap(PadButton::SITUATION);
        assert_eq!(self.step(&[]).state, StateId::Stylechange);
        for _ in 0..300 {
            if self.step(&[]).state != StateId::Stylechange {
                break;
            }
        }
    }

    /// A ground combo in the form, then two more full gauges (finish levels 2, 3).
    fn combo_and_levels(&mut self) {
        self.mark("combo");
        for _ in 0..150 {
            self.tap(PadButton::ATTACK);
        }
        self.idle(60);
        self.mark("finish levels 2 and 3");
        self.sora.debug_add_style(100.0);
        self.idle(3);
        self.sora.debug_add_style(100.0);
        self.idle(20);
    }

    /// Triangle on the finish command, then on every extra while STYLEFINISH runs; the steps it took.
    fn run_finish(&mut self) -> usize {
        self.mark("finish");
        self.tap(PadButton::SITUATION);
        for k in 1..=1500 {
            let press = self.sora.combat().situation.extra.is_some();
            let out = if press { self.tap(PadButton::SITUATION) } else { self.step(&[]) };
            if out.state != StateId::Stylefinish && self.sora.finish().is_none() {
                return k;
            }
        }
        panic!("STYLEFINISH did not end");
    }
}

// ------------------------------------------------------------------------------------ Second Form S (Starlight)

#[test]
fn second_form_s_reaches_ancient_light_and_runs_slot_155_to_the_end() {
    let mut b = Bench::new(holding(STARLIGHT, &[]), &[dummy(1, 300.0, 0.0)]);
    b.enter(0x95);
    let f = b.sora.combat().form;
    assert_eq!((f.cmd, f.level, f.wear_form, f.finish_level, f.finish_cmd), (0x95, 1, 5, 1, 0xAB), "Second Form S: Stun Impact first");
    assert!(f.weapon_units()[0].ends_with("w_so150_Wep"), "Starlight's own weapon: {:?}", f.weapon_units());
    assert!(b.sora.status().abilities.has(A::ComboMaster), "wear form 5's abilities (p_ex001_FormAbility)");
    b.combo_and_levels();
    let f = b.sora.combat().form;
    assert_eq!((f.finish_level, f.finish_cmd), (3, 0xAE), "SetFinishLevel 0x141414C70: 0xAE in form 0x95 (the level-2 hold is lifted)");
    assert_eq!(b.sora.combat().situation.slot_b.map(|e| e.kind), Some(0xAE), "Ancient Light in the finish slot");
    let ground_z = b.step(&[]).position.z;
    let from = b.trace.lines().count();
    let steps = b.run_finish();
    let lines: Vec<&str> = b.trace.lines().skip(from).collect();
    let finish: Vec<&str> = lines.iter().copied().filter(|l| l.contains("finish 0xae")).collect();
    assert!(!finish.is_empty(), "STYLEFINISH ran 0xAE");
    // sub 0 -> 3 (on the floor: a floor nearer than 500 cm) -> 1 -> 2 (the fifth pillar window) -> 0xC (TresEnableChange).
    let subs: Vec<u8> = finish.iter().filter_map(|l| l.split(" sub ").nth(1).and_then(|s| s.split(' ').next()).and_then(|s| s.parse().ok())).fold(Vec::new(), |mut v, s| {
        if v.last() != Some(&s) {
            v.push(s);
        }
        v
    });
    assert_eq!(subs, [3, 1, 2, 12], "the sub-steps of slot 155 ({steps} steps)");
    assert!(lines.iter().any(|l| l.contains("a=M_1435")), "the clip M_1435");
    // the rise (slot 164 / 165): 500 cm above the floor at once (dt x 500 > 1: the swept SetActorLocation), then the clip's own
    // 350 cm of root motion.
    let z = |l: &str| l.split(" z=").nth(1).and_then(|s| s.split(' ').next()).and_then(|s| s.parse::<f32>().ok()).unwrap();
    let top = finish.iter().map(|l| z(l)).fold(f32::MIN, f32::max);
    assert!(top > ground_z + 450.0, "Sora rose: top {top} from {ground_z}");
    // the hits: the pillars (LM_UNIONDISCODE, the Starlight weapon's L2_Pillar groups), the pull and the blast.
    let hits = b.host.hits();
    for row in ["LM_UNIONDISCODE", "LM_UNIONDISCODE2", "LM_UNIONDISCODE4"] {
        assert!(hits.iter().any(|(h, _)| h.target == TargetId(1) && h.attack_name == row), "{row} landed");
    }
    // the world slow ends with the fifth pillar window (+0x12C > 4), before TresEnableChange.
    let slow_end = finish.iter().position(|l| l.contains("slow false")).expect("the world slow ended");
    assert!(finish[..slow_end].iter().any(|l| l.contains("sub 1") || l.contains("sub 3")));
    // the form ends with the finish (EndStyle 0x94 .. 0x98: 0xAB .. 0xAE deleted).
    b.idle(30);
    let f = b.sora.combat().form;
    assert_eq!((f.cmd, f.wear_form, f.finish_level, f.ended), (0, 0, 0, 1));
    assert!(!b.kinds().contains(&0xAE) && b.sora.combat().situation.slot_b.is_none());
    b.save("l2_session.trace");
}

// ------------------------------------------------------------------------------------ Second Form D (Dead of Night)

#[test]
fn second_form_d_runs_second_form_to_last_arcanum() {
    let mut b = Bench::new(holding(DEAD_OF_NIGHT, &[]), &[dummy(1, 300.0, 0.0)]);
    b.enter(0x98);
    let f = b.sora.combat().form;
    assert_eq!((f.cmd, f.level, f.wear_form, f.finish_cmd), (0x98, 1, 5, 0xAB));
    assert!(f.weapon_units()[0].ends_with("w_so121_Wep"));
    b.combo_and_levels();
    let f = b.sora.combat().form;
    assert_eq!((f.finish_level, f.finish_cmd), (3, 0xAD), "Second Form D's level 3 is Last Arcanum (only form 0x95 has 0xAE)");
    let from = b.trace.lines().count();
    b.run_finish();
    let lines: Vec<&str> = b.trace.lines().skip(from).collect();
    assert!(lines.iter().any(|l| l.contains("finish 0xad")) && lines.iter().any(|l| l.contains("a=M_1427")), "Last Arcanum");
    assert!(b.host.hits().iter().any(|(h, _)| h.attack_name.starts_with("LM_LASTARCANUM")), "Last Arcanum hit");
    b.idle(30);
    let f = b.sora.combat().form;
    assert_eq!((f.cmd, f.wear_form, f.ended), (0, 0, 1));
    b.save("l5_session.trace");
}

// ------------------------------------------------------------------------------------ XS Ancient Light by Keyblade

/// A powerful foe locked on, a full gauge: what the queue offers, and what Y spent.
fn xs_offer(key: &str) -> (Vec<i32>, Option<Vec<i32>>) {
    let mut b = Bench::new(holding(key, &[A::AncientLight]), &[dummy(1, 250.0, 0.0)]);
    b.view = Some(kh_core::magic::View::from_camera(Vec3::new(-300.0, 0.0, 150.0), 0.0, 0.0, 90.0, 1280, 720));
    b.host.bodies.push((TargetId(1), TargetBody { reaction: BodyReaction::Strong, ..TargetBody::DEFAULT }));
    b.lock(1);
    b.sora.debug_add_style(100.0);
    b.idle(30);
    let offered = b.held();
    if !offered.contains(&174) {
        return (offered, None);
    }
    // Y on Ancient Light (the selected entry is the newer one, as in tests/xsi_abilities.rs).
    b.tap(PadButton::SITUATION);
    b.idle(1);
    assert_eq!(b.sora.combat().form.cmd, 0, "{key}: Ancient Light instead of the form");
    (offered, Some(b.held()))
}

#[test]
fn xs_ancient_light_is_offered_beside_each_keyblades_second_form() {
    for (key, cmd) in [(KINGDOM_KEY, 0x94), (STARLIGHT, 0x95), (DEAD_OF_NIGHT, 0x98)] {
        let (offered, after) = xs_offer(key);
        assert!(offered.contains(&cmd) && offered.contains(&174), "{key}: {offered:?}");
        let after = after.expect("used");
        assert!(!after.contains(&cmd) && !after.contains(&174), "{key}: the Second Form offer was spent: {after:?}");
    }
    // A Keyblade of another formchange (Shooting Star: Double Arrowguns) has no Second Form beside which to offer it.
    let (offered, after) = xs_offer("WEP_KEYBLADE_SO_02");
    assert!(after.is_none() && offered == [0x80], "{offered:?}");
}
