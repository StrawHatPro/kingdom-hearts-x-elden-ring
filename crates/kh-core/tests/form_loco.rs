//! (weapons F-LOCO) THE FORMS' LOCOMOTION through their anim sets: for every row of sheets/port/sora-keyblade-forms.json
//! (the 26 Keyblade forms and Rage Form) Sora is held in the form with the battle flag on and the trace records what the
//! Idle, Run and Jump states play (build/sora_tmp/weapons/F-LOCO/STATE.md). KH3: every play by name resolves through
//! m_ReplaceAnimSets (FindAnim 0x14056DE80, the last set first), so in a form the base set's `IdleB` / `RunLoopB` /
//! `RunStartB` ... are the form's rows (Blizzard Blades' skating `RunLoopB7`, a blend space), Second Form's are the base
//! ones. The trace is written to build/sora_tmp/weapons/F-LOCO/loco.trace.

use kh_core::anim::AnimId;
use kh_core::combat::form::KEYBLADE_FORMS;
use kh_core::combat::forms;
use kh_core::test_world::TestWorld;
use kh_core::{Input, Output, Pose, RecordingHost, Sora, StateId, Tuning, Vec2, Vec3, SORA_PARAMS};

const DT: f32 = 1.0 / 60.0;

/// The level-99 loadout (sheets/port/sora-loadout-99.json: the command deck, the abilities), no ability equipped.
fn loadout_99() -> kh_core::Status {
    use kh_core::status::LoadoutValue;
    let cell = |json: &serde_json::Value| -> Option<LoadoutValue> {
        match json {
            serde_json::Value::Number(n) => n.as_f64().map(LoadoutValue::Number),
            serde_json::Value::String(s) => Some(LoadoutValue::Text(s.clone())),
            serde_json::Value::Array(items) => items.iter().map(|i| i.as_str().map(str::to_string)).collect::<Option<Vec<_>>>().map(LoadoutValue::List),
            serde_json::Value::Object(map) => map.iter().map(|(k, v)| v.as_f64().map(|n| (k.clone(), n))).collect::<Option<Vec<_>>>().map(LoadoutValue::Map),
            _ => None,
        }
    };
    let path = format!("{}/../../sheets/port/sora-loadout-99.json", env!("CARGO_MANIFEST_DIR"));
    let json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))).unwrap();
    let mut loadout = kh_core::Loadout::new("sora-loadout-99");
    for row in json["rows"].as_array().unwrap() {
        let k = row[0].as_str().unwrap();
        let value = if k == "abilities.equipped" { LoadoutValue::List(Vec::new()) } else { cell(&row[1]).unwrap() };
        loadout.set(k, &value).unwrap_or_else(|e| panic!("{e}"));
    }
    loadout.resolve().unwrap();
    kh_core::Status::from_loadout(&loadout)
}

struct Bench {
    sora: Sora,
    host: RecordingHost,
    world: TestWorld,
}

impl Bench {
    fn in_form(cmd: i32, stage: i32, keyblade: &str, tuning: Tuning) -> Bench {
        let origin = Vec3::new(0.0, 0.0, Sora::origin_height_on_floor(&SORA_PARAMS));
        let mut sora = Sora::new(SORA_PARAMS, Pose { position: origin, yaw: 0.0 }).with_tuning(tuning);
        *sora.status_mut() = loadout_99();
        if keyblade != "-" {
            sora.status_mut().weapon = keyblade.to_string();
        }
        sora.force_battle(true);
        let f = &mut sora.combat_mut().form;
        (f.cmd, f.level, f.time_left, f.time_full) = (cmd, stage, 1.0e4, 1.0e4);
        Bench { sora, host: RecordingHost::new(), world: TestWorld::floor() }
    }

    fn step(&mut self, input: &Input) -> Output {
        let (sora, world) = (&mut self.sora, &self.world);
        self.host.step(world, |h| sora.step_hosted(DT, input, h))
    }

    /// The distinct clips (and, with the speed-matched loop, its run side) of `n` steps.
    fn run_for(&mut self, n: usize, input: &Input, clips: &mut Vec<&'static str>, states: &mut Vec<StateId>) {
        for _ in 0..n {
            let out = self.step(input);
            let mut push = |c: &'static str| {
                if clips.last() != Some(&c) {
                    clips.push(c);
                }
            };
            push(out.anim.name.name());
            if let Some(l) = out.anim.locomotion {
                push(l.run.name());
            }
            if states.last() != Some(&out.state) {
                states.push(out.state);
            }
        }
    }
}

/// The row a base name plays as in the form (the contract), for the expectations.
fn expect(cmd: i32, base: AnimId) -> &'static str {
    forms::replaced(cmd, base.name()).unwrap_or(base).name()
}

fn trace_form(row: &kh_core::combat::form::KeybladeFormRow, tuning: Tuning, trace: &mut String) {
    let mut b = Bench::in_form(row.cmd, row.stage, row.keyblade, tuning);
    let (mut idle, mut run, mut jump, mut states) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    b.run_for(90, &Input::default(), &mut idle, &mut states);
    assert_eq!(b.sora.combat().form.cmd, row.cmd, "{}: the form is held", row.id);
    let forward = Input { move_stick: Vec2::new(0.0, 1.0), ..Input::default() };
    b.run_for(120, &forward, &mut run, &mut states);
    // The blend space plays as one: a curve to the right leans the run (its axis, the delta yaw), the samples drawn are
    // the form's; with the speed-matched loop, the walk / run pair and its speed weight.
    let mut lean = String::new();
    let mut weights = (f32::MAX, f32::MIN);
    for k in 0..70 {
        // 40 steps at a walk's stick (the speed falls, the loop's run weight with it), then 30 back to a run, curving right
        let stick = if k < 40 { Vec2::new(0.0, 0.4) } else { Vec2::new(0.45, 0.9) };
        let out = b.step(&Input { move_stick: stick, ..Input::default() });
        if let Some(l) = out.anim.locomotion {
            weights = (weights.0.min(l.run_weight), weights.1.max(l.run_weight));
        }
        if k == 46 {
            let [(s0, w0, _), (s1, w1, _)] = out.anim.body_samples();
            lean = format!("{} axis {:.1}: {s0} x {w0:.2} + {s1} x {w1:.2}{}", out.anim.name.name(), out.anim.axis, out.anim.locomotion.map_or(String::new(), |l| format!(" (speed-matched loop: walk {} / run {}, run weight {:.2})", l.walk.name(), l.run.name(), l.run_weight)));
        }
    }
    b.run_for(30, &Input::default(), &mut run, &mut states);
    let press = Input { jump_pressed: true, jump_held: true, ..Input::default() };
    b.step(&press);
    b.run_for(100, &Input::default(), &mut jump, &mut states);
    let sets: Vec<&str> = row.anim_sets.iter().map(|s| s.rsplit('.').next().unwrap_or(s)).collect();
    let speed = if weights.0 <= weights.1 { format!(", run weight {:.2} .. {:.2} by speed", weights.0, weights.1) } else { String::new() };
    trace.push_str(&format!("{} {:#04X} {} [{}]\n  idle: {}\n  run:  {}\n  lean: {lean}{speed}\n  jump: {}\n  states: {:?}\n", row.id, row.cmd, row.name, sets.join(" "), idle.join(" "), run.join(" "), jump.join(" "), states));
    // The Idle state shows the form's IdleB (the battle pose), the Run state its RunLoopB (with the speed-matched loop:
    // its run side), both from the chain; the rows are the base ones for Second Form.
    let (idle_b, run_b) = (expect(row.cmd, AnimId::IdleB), expect(row.cmd, AnimId::RunLoopB));
    assert!(idle.contains(&idle_b), "{}: the idle plays {idle_b}: {idle:?}", row.id);
    assert!(run.contains(&run_b), "{}: the run plays {run_b}: {run:?}", row.id);
    if row.anim_sets.is_empty() {
        assert_eq!((idle_b, run_b), ("IdleB", "RunLoopB"), "{}: Second Form plays the base set", row.id);
    }
}

#[test]
fn every_form_plays_its_own_locomotion() {
    let mut trace = String::from("# (F-LOCO) the locomotion clips per form (Tuning::GAME_EXACT, the battle flag on), from crates/kh-core/tests/form_loco.rs\n");
    for row in KEYBLADE_FORMS {
        trace_form(row, Tuning::GAME_EXACT, &mut trace);
    }
    trace.push_str("\n# the user's options (Tuning::USER: the speed-matched locomotion loop)\n");
    for row in KEYBLADE_FORMS {
        trace_form(row, Tuning::USER, &mut trace);
    }
    let out = format!("{}/../../build/sora_tmp/weapons/F-LOCO/loco.trace", env!("CARGO_MANIFEST_DIR"));
    if let Some(dir) = std::path::Path::new(&out).parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(&out, &trace);
    assert_eq!(KEYBLADE_FORMS.len(), 27, "26 Keyblade forms and Rage Form");
}

/// The pitch toward the target (Attack +0x1F4 bit 5): Twin Yo-Yos' Anchor Shot (kind 0x96, M_4415) at a far target that
/// stands 3 m higher pitches the actor up (Facing 0x14134D0E0: the rotation of the 3D direction), and the attack's End
/// (0x141335120) turns him back to pitch 0. Every other kind keeps pitch 0.
#[test]
fn anchor_shot_pitches_him_toward_a_high_target() {
    use kh_core::{PadButton, PadButtons, PadState, TargetFlags, TargetId, TargetInfo};
    let mut b = Bench::in_form(0x82, 2, "WEP_KEYBLADE_SO_05", Tuning::GAME_EXACT);
    let (x, z) = (700.0, 300.0);
    b.host.targets.push(TargetInfo { id: TargetId(1), position: Vec3::new(x, 0.0, z), center: Vec3::new(x, 0.0, z + 90.0), velocity: Vec3::ZERO, yaw: 180.0, radius: 40.0, half_height: 90.0, flags: TargetFlags(TargetFlags::ALIVE | TargetFlags::HITTABLE | TargetFlags::LOCKABLE | TargetFlags::ENEMY), hp: 1.0e6, hp_max: 1.0e6 });
    for _ in 0..30 {
        b.step(&Input::default());
    }
    let attack = Input::default().with_pad(PadState::from_held(PadButtons::NONE.with(PadButton::ATTACK), PadButtons::NONE));
    b.step(&attack);
    let (mut most, mut anchor, mut after) = (0.0f32, false, None);
    for _ in 0..240 {
        let out = b.step(&Input::default());
        anchor |= b.sora.combat().history.last() == Some(&0x96);
        most = most.max(out.pitch);
        if anchor && matches!(out.state, StateId::Idle | StateId::Run | StateId::Jump) {
            after = Some(out.pitch);
            break;
        }
    }
    assert!(anchor, "far: Anchor Shot (0x96): {:?} state {:?} lock {:?}", b.sora.combat().history, b.sora.output().state, b.sora.combat().lockon.current);
    // atan(300 / 700) = 23.2 degrees between the capsule centres; the turn eases toward it at dt x 12
    assert!(most > 10.0 && most < 30.0, "pitched up toward the high target: {most}");
    assert_eq!(after, Some(0.0), "End turned him back to pitch 0");
}
