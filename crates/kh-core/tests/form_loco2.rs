//! (FORM-LOCO2) THE USER'S RULE (2026-11-06): "ALL forms, when transformed into them but not in combat, need to keep using
//! the combat animations of the form until they end". For every row of sheets/port/sora-keyblade-forms.json (26 Keyblade
//! forms and Rage Form) Sora is held in the form IN battle, then the battle flag drops (the 1.0 s linger runs out, no enemy
//! engaged) and he idles, walks, runs, turns round and jumps OUT of battle: every clip is the form's battle set (IdleB,
//! WalkLoopB, RunLoopB, RunStartB, LandB through the form's anim sets), never the normal one (IdleN, BattleEnd,
//! RunBattleEnd, the `N` / `W` loops), and the form weapon stays out. Then the form runs out and the normal rule resumes
//! (BattleEnd, IdleN). Kernel: `combat::forms::holds_battle_stance` (Idle / Run's battle flag) and `actions::battle_anim`
//! (KH3's 0x141435A10). The trace goes to build/sora_tmp/weapons/FORM-LOCO2/out_of_combat.trace.

use kh_core::anim::AnimId;
use kh_core::combat::form::{is_second_form, KeybladeFormRow, KEYBLADE_FORMS};
use kh_core::combat::forms;
use kh_core::test_world::TestWorld;
use kh_core::{Input, Output, Pose, RecordingHost, Sora, StateId, Tuning, Vec2, Vec3, SORA_PARAMS};

const DT: f32 = 1.0 / 60.0;

struct Bench {
    sora: Sora,
    host: RecordingHost,
    world: TestWorld,
}

impl Bench {
    fn in_form(row: &KeybladeFormRow) -> Bench {
        let origin = Vec3::new(0.0, 0.0, Sora::origin_height_on_floor(&SORA_PARAMS));
        let mut sora = Sora::new(SORA_PARAMS, Pose { position: origin, yaw: 0.0 }).with_tuning(Tuning::GAME_EXACT);
        if row.keyblade != "-" {
            sora.status_mut().weapon = row.keyblade.to_string();
        }
        sora.force_battle(true);
        let f = &mut sora.combat_mut().form;
        (f.cmd, f.level, f.time_left, f.time_full) = (row.cmd, row.stage, 1.0e4, 1.0e4);
        Bench { sora, host: RecordingHost::new(), world: TestWorld::floor() }
    }

    fn step(&mut self, input: &Input) -> Output {
        let (sora, world) = (&mut self.sora, &self.world);
        self.host.step(world, |h| sora.step_hosted(DT, input, h))
    }

    /// The distinct clips (and states) of `n` steps.
    fn run_for(&mut self, n: usize, input: &Input, clips: &mut Vec<&'static str>, states: &mut Vec<StateId>) {
        for _ in 0..n {
            let out = self.step(input);
            let c = out.anim.name.name();
            if clips.last() != Some(&c) {
                clips.push(c);
            }
            if states.last() != Some(&out.state) {
                states.push(out.state);
            }
        }
    }
}

/// The row a base name plays as in the form (F-LOCO's contract).
fn expect(cmd: i32, base: AnimId) -> &'static str {
    forms::replaced(cmd, base.name()).unwrap_or(base).name()
}

/// The clips of the normal (out of battle) set, as they would play in the form (less the rows a form's set maps both a
/// normal and a battle name to: Counter Shield's RunStartB11 serves RunStartN / W / B).
fn normal_clips(cmd: i32) -> Vec<&'static str> {
    let battle: Vec<&str> = [AnimId::IdleB, AnimId::IdleLockon, AnimId::BattleStart, AnimId::RunLoopB, AnimId::WalkLoopB, AnimId::RunStartB, AnimId::LandB, AnimId::LightLandB].iter().map(|&id| expect(cmd, id)).collect();
    let mut v = Vec::new();
    for id in [AnimId::IdleN, AnimId::BattleEnd, AnimId::IdleBreakN, AnimId::IdleBreakW, AnimId::RunBattleEnd, AnimId::RunLoopN, AnimId::RunLoopW, AnimId::WalkLoopN, AnimId::WalkLoopW, AnimId::RunStartN, AnimId::RunStartW, AnimId::Land, AnimId::LightLand] {
        for name in [id.name(), expect(cmd, id)] {
            if !v.contains(&name) && !battle.contains(&name) {
                v.push(name);
            }
        }
    }
    v
}

fn trace_form(row: &KeybladeFormRow, trace: &mut String) {
    let mut b = Bench::in_form(row);
    let cmd = row.cmd;
    let held = forms::holds_battle_stance(cmd);
    let (mut fight, mut idle, mut walk, mut run, mut jump, mut after, mut states) = (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
    // In battle (the trace stays as before: the form's battle idle). The bench sets the form without StyleChange, so a hop
    // first gives the Idle Begin that follows a transformation (it draws the weapon when `battleAnim` is on: Rage Form).
    let press = Input { jump_pressed: true, jump_held: true, ..Input::default() };
    b.step(&press);
    b.run_for(100, &Input::default(), &mut Vec::new(), &mut states);
    b.run_for(90, &Input::default(), &mut fight, &mut states);
    assert!(fight.contains(&expect(cmd, AnimId::IdleB)), "{}: in battle the idle is {}: {fight:?}", row.id, expect(cmd, AnimId::IdleB));
    // Out of battle: the forced flag off, no enemy: the 1.0 s linger runs out, then 6 s of idling (KH3's plain-Keyblade
    // rule would play BattleEnd at the end of the idle clip, its 3.0 s countdown for a form kind without the weapon out).
    b.sora.force_battle(false);
    b.run_for(60 + 360, &Input::default(), &mut idle, &mut states);
    assert!(!b.sora.actions().battle, "{}: out of battle", row.id);
    let walk_in = Input { move_stick: Vec2::new(0.0, 0.5), ..Input::default() };
    b.run_for(90, &walk_in, &mut walk, &mut states);
    let forward = Input { move_stick: Vec2::new(0.0, 1.0), ..Input::default() };
    b.run_for(120, &forward, &mut run, &mut states);
    // a flick to the opposite direction: the turn-around (RunStart with the delta yaw)
    let back = Input { move_stick: Vec2::new(0.0, -1.0), ..Input::default() };
    b.run_for(60, &back, &mut run, &mut states);
    b.run_for(40, &Input::default(), &mut run, &mut states);
    let press = Input { jump_pressed: true, jump_held: true, ..Input::default() };
    b.step(&press);
    b.run_for(150, &Input::default(), &mut jump, &mut states);
    let weapon_out = b.sora.actions().weapon_out;
    // The form ends (its time runs out): the normal rule again.
    b.sora.combat_mut().form.time_left = 0.02;
    let mut ended_at = None;
    for k in 0..600 {
        let out = b.step(&Input::default());
        let c = out.anim.name.name();
        if after.last() != Some(&c) {
            after.push(c);
        }
        if b.sora.combat().form.cmd == 0 {
            ended_at = Some(k);
            break;
        }
    }
    let ended = ended_at.is_some();
    let mut normal = Vec::new();
    if ended {
        b.run_for(900, &Input::default(), &mut normal, &mut states);
    }
    let sets: Vec<&str> = row.anim_sets.iter().map(|s| s.rsplit('.').next().unwrap_or(s)).collect();
    trace.push_str(&format!(
        "{} {:#04X} {} [{}] holds={held}\n  battle: {}\n  idle:   {}\n  walk:   {}\n  run:    {}\n  jump:   {}\n  weapon out: {weapon_out}\n  end:    {} (form ended after {ended_at:?} steps)\n  after:  {}\n",
        row.id,
        cmd,
        row.name,
        sets.join(" "),
        fight.join(" "),
        idle.join(" "),
        walk.join(" "),
        run.join(" "),
        jump.join(" "),
        after.join(" "),
        normal.join(" ")
    ));
    // (the coordinator, 2026-11-06) every row holds it: Second Form (kind 1), Light Form (0x16), Dark Form (0x17) too
    assert!(held, "{}: holds the battle stance{}", row.id, if is_second_form(cmd) { " (Second Form is included)" } else { "" });
    let bad = normal_clips(cmd);
    for (what, clips) in [("idle", &idle), ("walk", &walk), ("run", &run), ("jump", &jump)] {
        let wrong: Vec<&&str> = clips.iter().filter(|c| bad.contains(c)).collect();
        assert!(wrong.is_empty(), "{}: out of battle the {what} played the normal set {wrong:?}: {clips:?}", row.id);
    }
    assert_eq!(idle.last(), Some(&expect(cmd, AnimId::IdleB)), "{}: out of battle the idle is the form's IdleB: {idle:?}", row.id);
    assert!(run.contains(&expect(cmd, AnimId::RunLoopB)), "{}: out of battle the run is the form's {}: {run:?}", row.id, expect(cmd, AnimId::RunLoopB));
    assert!(walk.contains(&expect(cmd, AnimId::WalkLoopB)), "{}: out of battle the walk is the form's {}: {walk:?}", row.id, expect(cmd, AnimId::WalkLoopB));
    assert!(weapon_out, "{}: the form weapon stays out", row.id);
    // After the end: the base (no form) rule runs again: the battle pose is left (BattleEnd, then IdleN).
    assert!(ended, "{}: the form ran out: {after:?}", row.id);
    assert!(normal.contains(&"IdleN"), "{}: after the form the normal idle resumes: {normal:?}", row.id);
}

#[test]
fn every_form_keeps_its_battle_set_out_of_combat_until_it_ends() {
    let mut trace = String::from("# (FORM-LOCO2) every form in and out of battle, then its end (Tuning::GAME_EXACT), from crates/kh-core/tests/form_loco2.rs\n");
    for row in KEYBLADE_FORMS {
        trace_form(row, &mut trace);
    }
    // Light Form / Dark Form (weapon kinds 0x16 / 0x17, which KH3's battleAnim treats like the plain Keyblade) and the
    // Second Form family (kind 1) are held: they rely on the forced battle flag alone
    for cmd in [0xDD, 0xDE, 0x94, 0x95, 0x96, 0x97, 0x98] {
        assert!(forms::holds_battle_stance(cmd), "{cmd:#04X} holds the battle stance");
    }
    let out = format!("{}/../../build/sora_tmp/weapons/FORM-LOCO2/out_of_combat.trace", env!("CARGO_MANIFEST_DIR"));
    if let Some(dir) = std::path::Path::new(&out).parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(&out, &trace);
    assert_eq!(KEYBLADE_FORMS.len(), 27, "26 Keyblade forms and Rage Form");
}

/// The level-99 loadout (sheets/port/sora-loadout-99.json), no ability equipped, holding `keyblade`.
fn level99(keyblade: &str) -> kh_core::Status {
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
        let value = match k {
            "abilities.equipped" => LoadoutValue::List(Vec::new()),
            "weapon" => LoadoutValue::Text(keyblade.to_string()),
            _ => cell(&row[1]).unwrap(),
        };
        loadout.set(k, &value).unwrap_or_else(|e| panic!("{e}"));
    }
    loadout.resolve().unwrap();
    kh_core::Status::from_loadout(&loadout)
}

/// THE REAL FLOW, NO ENEMY AT ALL (the user's pictures): a full gauge offers the Keyblade's form, Triangle transforms him
/// (StyleChange, then Idle's Begin), and he idles and runs with the battle flag never on. `stages`: the forms entered in
/// turn (Double Form = Light Form, then the gauge again in it). Returns the trace lines.
fn by_the_gauge(keyblade: &str, stages: &[i32]) -> String {
    use kh_core::{PadButton, PadButtons, PadState};
    let origin = Vec3::new(0.0, 0.0, Sora::origin_height_on_floor(&SORA_PARAMS));
    let mut b = Bench { sora: Sora::new(SORA_PARAMS, Pose { position: origin, yaw: 0.0 }).with_tuning(Tuning::GAME_EXACT).with_status(level99(keyblade)), host: RecordingHost::new(), world: TestWorld::floor() };
    let (mut states, mut log) = (Vec::new(), String::new());
    b.run_for(5, &Input::default(), &mut Vec::new(), &mut states);
    let triangle = Input::default().with_pad(PadState::from_held(PadButtons::NONE.with(PadButton::SITUATION), PadButtons::NONE));
    for &cmd in stages {
        b.sora.debug_add_style(60.0);
        b.run_for(2, &Input::default(), &mut Vec::new(), &mut states);
        b.sora.debug_add_style(40.0);
        b.run_for(15, &Input::default(), &mut Vec::new(), &mut states);
        b.step(&triangle);
        let mut change = Vec::new();
        b.run_for(330, &Input::default(), &mut change, &mut states);
        assert_eq!(b.sora.combat().form.cmd, cmd, "{keyblade}: the gauge gave {cmd:#04X}: {change:?} {states:?}");
        log.push_str(&format!("  {cmd:#04X} transform: {}\n", change.join(" ")));
    }
    let cmd = b.sora.combat().form.cmd;
    let (mut idle, mut run) = (Vec::new(), Vec::new());
    b.run_for(360, &Input::default(), &mut idle, &mut states);
    let forward = Input { move_stick: Vec2::new(0.0, 1.0), ..Input::default() };
    b.run_for(150, &forward, &mut run, &mut states);
    b.run_for(60, &Input::default(), &mut run, &mut states);
    assert!(!b.sora.actions().battle, "{keyblade}: never in battle");
    log.push_str(&format!("  {cmd:#04X} out of battle (battle flag {}), weapon out {}\n    idle: {}\n    run:  {}\n", b.sora.actions().battle, b.sora.actions().weapon_out, idle.join(" "), run.join(" ")));
    let bad = normal_clips(cmd);
    let wrong: Vec<&&str> = idle.iter().chain(&run).filter(|c| bad.contains(c)).collect();
    assert!(wrong.is_empty(), "{keyblade}: out of battle the normal set {wrong:?}: idle {idle:?} run {run:?}");
    assert_eq!(idle.last(), Some(&expect(cmd, AnimId::IdleB)), "{keyblade}: idle {idle:?}");
    assert!(run.contains(&expect(cmd, AnimId::RunLoopB)), "{keyblade}: run {run:?}");
    assert!(b.sora.actions().weapon_out, "{keyblade}: the form weapon stays out");
    log
}

/// The user's two reports: Counter Shield (Hero's Origin) and Double Form (Oathkeeper: Light Form, then Double Form) by
/// the gauge with no enemy anywhere.
#[test]
fn counter_shield_and_double_form_out_of_combat_by_the_real_flow() {
    let mut trace = String::from("# (FORM-LOCO2) the real flow (gauge, Triangle, StyleChange), no enemy: crates/kh-core/tests/form_loco2.rs\n");
    trace.push_str("Counter Shield (WEP_KEYBLADE_SO_01)\n");
    trace.push_str(&by_the_gauge("WEP_KEYBLADE_SO_01", &[0x89]));
    trace.push_str("Double Form (WEP_KEYBLADE_SO_12: Light Form, then Double Form)\n");
    trace.push_str(&by_the_gauge("WEP_KEYBLADE_SO_12", &[0xDD, 0xDF]));
    trace.push_str("Light Form (WEP_KEYBLADE_SO_12) / Dark Form (WEP_KEYBLADE_SO_13)\n");
    trace.push_str(&by_the_gauge("WEP_KEYBLADE_SO_12", &[0xDD]));
    trace.push_str(&by_the_gauge("WEP_KEYBLADE_SO_13", &[0xDE]));
    trace.push_str("Second Form (WEP_KEYBLADE_SO_00, the Kingdom Key)\n");
    trace.push_str(&by_the_gauge("WEP_KEYBLADE_SO_00", &[0x94]));
    let out = format!("{}/../../build/sora_tmp/weapons/FORM-LOCO2/real_flow.trace", env!("CARGO_MANIFEST_DIR"));
    let _ = std::fs::write(&out, &trace);
}

/// No form: the rule changes nothing (the battle linger, BattleEnd, IdleN as before).
#[test]
fn without_a_form_the_battle_end_is_unchanged() {
    assert!(!forms::holds_battle_stance(0));
    let origin = Vec3::new(0.0, 0.0, Sora::origin_height_on_floor(&SORA_PARAMS));
    let mut b = Bench { sora: Sora::new(SORA_PARAMS, Pose { position: origin, yaw: 0.0 }).with_tuning(Tuning::GAME_EXACT), host: RecordingHost::new(), world: TestWorld::floor() };
    b.sora.force_battle(true);
    let (mut clips, mut states) = (Vec::new(), Vec::new());
    b.run_for(90, &Input::default(), &mut clips, &mut states);
    assert!(clips.contains(&"IdleB"), "{clips:?}");
    b.sora.force_battle(false);
    b.run_for(900, &Input::default(), &mut clips, &mut states);
    assert!(clips.contains(&"BattleEnd") && clips.last() != Some(&"IdleB"), "{clips:?}");
}
