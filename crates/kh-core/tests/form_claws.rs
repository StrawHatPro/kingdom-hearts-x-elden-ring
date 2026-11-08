//! (weapons F-CLAWS) Agile Claws -> Twin Yo-Yos (Happy Gear, CHANGE_CL 0x8A -> CHANGE_YO 0x82) and Blizzard Claws ->
//! Blizzard Blades (Crystal Snow, CHANGE_CL2 0x90 -> CHANGE_SK 0x85): the trace of each chain from the offer to its end
//! (build/sora_tmp/weapons/F-CLAWS/STATE.md): offer -> STYLECHANGE (StyleChange12 / StyleChange18, the wear form, the
//! claws) -> the COMBO_CL combo (0x88 0x8A 0x89 0x8B 0x8C, the hits on the dummy) -> the full gauge offers the second stage
//! -> STYLECHANGE (StyleChange4 / StyleChange7) -> COMBO_YO (0x92 0x93 0x94 0x95 0x9B 0x9A) / COMBO_SK (0xAA 0xAB 0xAC 0xAE
//! 0xB1) -> FINISH_YO (M_4420) / FINISH_SK (M_7419, the Chandelier's M_7423, M_1501) -> the form's end.

use kh_core::host::EventKind;
use kh_core::status::LoadoutValue;
use kh_core::test_world::TestWorld;
use kh_core::{Input, Loadout, Output, PadButton, PadButtons, PadState, Pose, RecordingHost, Sora, StateId, Status, TargetFlags, TargetId, TargetInfo, Tuning, Vec3, SORA_PARAMS};

const DT: f32 = 1.0 / 60.0;

fn cell(json: &serde_json::Value) -> Option<LoadoutValue> {
    match json {
        serde_json::Value::Number(n) => n.as_f64().map(LoadoutValue::Number),
        serde_json::Value::String(s) => Some(LoadoutValue::Text(s.clone())),
        serde_json::Value::Array(items) => items.iter().map(|i| i.as_str().map(str::to_string)).collect::<Option<Vec<_>>>().map(LoadoutValue::List),
        serde_json::Value::Object(map) => map.iter().map(|(k, v)| v.as_f64().map(|n| (k.clone(), n))).collect::<Option<Vec<_>>>().map(LoadoutValue::Map),
        _ => None,
    }
}

fn holding(key: &str) -> Status {
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
    s
}

fn dummy(id: u32, x: f32) -> TargetInfo {
    TargetInfo { id: TargetId(id), position: Vec3::new(x, 0.0, 0.0), center: Vec3::new(x, 0.0, 90.0), velocity: Vec3::ZERO, yaw: 180.0, radius: 40.0, half_height: 90.0, flags: TargetFlags(TargetFlags::ALIVE | TargetFlags::HITTABLE | TargetFlags::LOCKABLE | TargetFlags::ENEMY), hp: 1.0e6, hp_max: 1.0e6 }
}

struct Bench {
    sora: Sora,
    host: RecordingHost,
    world: TestWorld,
    /// Every step: (state, sub-step, clip, the events).
    log: Vec<(StateId, u8, &'static str, Vec<EventKind>)>,
}

impl Bench {
    fn new(key: &str) -> Bench {
        let origin = Vec3::new(0.0, 0.0, Sora::origin_height_on_floor(&SORA_PARAMS));
        let sora = Sora::new(SORA_PARAMS, Pose { position: origin, yaw: 0.0 }).with_tuning(Tuning::GAME_EXACT).with_status(holding(key));
        let mut host = RecordingHost::new();
        host.targets.push(dummy(1, 200.0));
        let mut b = Bench { sora, host, world: TestWorld::floor(), log: Vec::new() };
        b.idle(5);
        b
    }

    fn step(&mut self, held: &[PadButton]) -> Output {
        let set = held.iter().fold(PadButtons::NONE, |all, b| all.with(*b));
        let input = Input::default().with_pad(PadState::from_held(set, PadButtons::NONE));
        let (sora, world) = (&mut self.sora, &self.world);
        let out = self.host.step(world, |h| sora.step_hosted(DT, &input, h));
        self.log.push((out.state, out.sub_step, out.anim.name.name(), out.events.iter().map(|e| e.kind).collect()));
        out
    }

    fn idle(&mut self, n: usize) {
        for _ in 0..n {
            self.step(&[]);
        }
    }

    fn clips(&self, from: usize) -> Vec<&'static str> {
        let mut out: Vec<&'static str> = Vec::new();
        for (_, _, clip, _) in &self.log[from..] {
            if out.last() != Some(clip) {
                out.push(clip);
            }
        }
        out
    }

    fn states(&self, from: usize) -> Vec<StateId> {
        let mut out: Vec<StateId> = Vec::new();
        for (s, ..) in &self.log[from..] {
            if out.last() != Some(s) {
                out.push(*s);
            }
        }
        out
    }

    /// The offer, Triangle, the transformation; returns the step the form began at.
    fn transform(&mut self, cmd: i32) -> usize {
        self.sora.debug_add_style(100.0);
        self.idle(15);
        // the offer is selectable after its appear time (0.2 s) and start delay
        for _ in 0..120 {
            if self.sora.combat().situation.list.iter().any(|e| e.kind == cmd && e.appear <= 0.0 && e.start_delay <= 0.0) {
                break;
            }
            self.step(&[]);
        }
        let list: Vec<i32> = self.sora.combat().situation.list.iter().map(|e| e.kind).collect();
        assert!(list.contains(&cmd), "the formchange {cmd:#X} is offered: {list:?}");
        let from = self.log.len();
        self.step(&[PadButton::SITUATION]);
        let out = self.step(&[]);
        assert_eq!(out.state, StateId::Stylechange, "list {list:?}, form {:#X} left {} cd {} cia {:#X}, situation {:?} a {:?} b {:?}", self.sora.combat().form.cmd, self.sora.combat().form.time_left, self.sora.combat().command_cooldown, self.sora.combat().form.command_in_action, self.sora.combat().situation.list.iter().map(|e| e.kind).collect::<Vec<_>>(), self.sora.combat().situation.slot_a.map(|e| e.kind), self.sora.combat().situation.slot_b.map(|e| e.kind));
        for _ in 0..400 {
            if self.step(&[]).state != StateId::Stylechange {
                return from;
            }
        }
        panic!("STYLECHANGE did not end");
    }

    /// Attack presses until the chain ends (back to IDLE / RUN / JUMP); the kinds played.
    fn combo(&mut self) -> Vec<u8> {
        let first = self.sora.combat().history.len();
        for k in 0..900 {
            let out = if k % 2 == 0 { self.step(&[PadButton::ATTACK]) } else { self.step(&[]) };
            if k > 30 && matches!(out.state, StateId::Idle | StateId::Run | StateId::Jump) {
                break;
            }
        }
        self.sora.combat().history[first..].to_vec()
    }

    /// Triangle on the finish command, then until STYLEFINISH ends; the sub-steps.
    fn finish(&mut self) -> Vec<u8> {
        self.step(&[PadButton::SITUATION]);
        let mut subs = Vec::new();
        for _ in 0..3000 {
            let out = self.step(&[]);
            match self.sora.finish() {
                Some(f) if subs.last() != Some(&f.sub) => subs.push(f.sub),
                Some(_) => {}
                None if out.state != StateId::Stylefinish => break,
                None => {}
            }
        }
        subs
    }

    fn hp(&self) -> f32 {
        self.host.targets[0].hp
    }
}

/// The weapons the renderer draws for the held form (the row's units).
fn units(b: &Bench) -> Vec<&'static str> {
    b.sora.combat().form.weapon_units().iter().map(|u| u.rsplit('/').next().unwrap_or("")).collect()
}

#[test]
fn agile_claws_to_twin_yo_yos_from_the_offer_to_the_end() {
    let mut b = Bench::new("WEP_KEYBLADE_SO_05");
    for id in ["form.CL", "form.YO"] {
        assert_eq!(kh_core::combat::forms::moveset_name(id), "claws");
    }
    // ---- offer -> Agile Claws
    let from = b.transform(0x8A);
    let f = b.sora.combat().form;
    assert_eq!((f.cmd, f.level, f.time_full, f.wear_form), (0x8A, 1, 30.0, 1), "Agile Claws: stage 1, 30 s, wear form 1 (Brave)");
    assert!(b.clips(from).contains(&"StyleChange12"), "{:?}", b.clips(from));
    assert_eq!(units(&b), ["w_so062_Wep", "w_so063_Wep"]);
    // ---- the combo: COMBO_CL near one target
    b.idle(30);
    let hp = b.hp();
    let from = b.log.len();
    let kinds = b.combo();
    assert_eq!(kinds, [0x88, 0x8A, 0x89, 0x8B, 0x8C], "the claws near one target");
    assert!(b.states(from).contains(&StateId::ComboCl), "{:?}", b.states(from));
    for c in ["C411", "C413", "C412", "C414", "C415"] {
        assert!(b.clips(from).contains(&c), "{c}: {:?}", b.clips(from));
    }
    assert!(b.hp() < hp, "the claws' windows hit the dummy through the generic hit path ({hp} -> {})", b.hp());
    // ---- the full gauge in Agile Claws offers Twin Yo-Yos (no finish command of its own)
    b.idle(30);
    let from = b.transform(0x82);
    let f = b.sora.combat().form;
    assert_eq!((f.cmd, f.level, f.time_full), (0x82, 2, 25.0), "Twin Yo-Yos: stage 2, 25 s");
    assert!(b.clips(from).contains(&"StyleChange4"), "{:?}", b.clips(from));
    assert_eq!(units(&b), ["w_so064_Wep", "w_so065_Wep"]);
    assert_eq!(b.sora.combat().situation.slot_b.map(|e| e.kind), Some(0x9C), "FINISH_YO on offer");
    // ---- COMBO_YO
    b.idle(30);
    let hp = b.hp();
    let from = b.log.len();
    let kinds = b.combo();
    assert_eq!(kinds, [0x92, 0x93, 0x94, 0x95, 0x9B, 0x9A], "the yo-yos near one target");
    assert!(b.states(from).contains(&StateId::ComboYo));
    assert!(b.hp() < hp, "the yo-yos hit");
    // ---- FINISH_YO: M_4420, then the form ends
    b.idle(10);
    let from = b.log.len();
    let hp = b.hp();
    let subs = b.finish();
    assert_eq!(subs.first(), Some(&1), "{subs:?}");
    assert!(subs.contains(&5), "{subs:?}");
    assert!(b.clips(from).contains(&"4420"), "{:?}", b.clips(from));
    assert!(b.hp() < hp, "the giant yo-yo hits");
    b.idle(5);
    assert_eq!(b.sora.combat().form.cmd, 0, "the form ended with its finish");
}

#[test]
fn anchor_shot_pulls_him_to_a_far_target() {
    let mut b = Bench::new("WEP_KEYBLADE_SO_05");
    b.transform(0x8A);
    b.idle(30);
    b.transform(0x82);
    b.idle(30);
    b.host.targets[0] = dummy(1, 700.0);
    b.idle(5);
    let x0 = b.sora.output().position.x;
    b.step(&[PadButton::ATTACK]);
    b.step(&[]);
    assert_eq!(b.sora.combat().history.last(), Some(&0x96), "far: Anchor Shot");
    let mut x1 = x0;
    for _ in 0..120 {
        let o = b.step(&[]);
        if o.anim.name.name() == "4416" {
            // (F-POLISH) KH3's frames: the handler starts M_4416 at the hit (frame N) and sets pawn+0x1560; the pawn tick of
            // frame N + 1 puts him there.
            assert!((o.position.x - x0).abs() < 150.0, "not yet moved by the warp at the hit's frame: {x0} -> {}", o.position.x);
            x1 = b.step(&[]).position.x;
            break;
        }
    }
    // (the pull is the pawn's previous kind 0x97 and M_4416: PlayAttack's history is not written by the handler)
    assert!(x1 > x0 + 300.0 && x1 < 700.0, "pulled up to the target (slot 136: where the line from him meets its body): {x0} -> {x1}");
}

#[test]
fn blizzard_claws_to_blizzard_blades_from_the_offer_to_the_end() {
    let mut b = Bench::new("WEP_KEYBLADE_SO_06");
    // ---- offer -> Blizzard Claws: the claws' class and kinds, its own weapons, the ice effect group, the CL2 rows
    let from = b.transform(0x90);
    let f = b.sora.combat().form;
    assert_eq!((f.cmd, f.level, f.wear_form), (0x90, 1, 4), "Blizzard Claws: stage 1, wear form 4 (Grace)");
    assert!(b.clips(from).contains(&"StyleChange18"), "{:?}", b.clips(from));
    assert_eq!(units(&b), ["w_so082_Wep", "w_so083_Wep"]);
    b.idle(30);
    let hp = b.hp();
    let from = b.log.len();
    assert_eq!(b.combo(), [0x88, 0x8A, 0x89, 0x8B, 0x8C]);
    assert!(b.states(from).contains(&StateId::ComboCl));
    assert!(b.hp() < hp);
    // ---- Blizzard Blades
    b.idle(30);
    let from = b.transform(0x85);
    let f = b.sora.combat().form;
    assert_eq!((f.cmd, f.level), (0x85, 2));
    assert!(b.clips(from).contains(&"StyleChange7"), "{:?}", b.clips(from));
    assert_eq!(units(&b), ["w_so084_Wep", "w_so085_Wep", "w_so086_Wep", "w_so087_Wep"], "four blades on forearms and shins");
    assert_eq!(b.sora.combat().situation.slot_b.map(|e| e.kind), Some(0x9F), "FINISH_SK on offer");
    b.idle(30);
    let hp = b.hp();
    let from = b.log.len();
    // the first ground step (M_7411, SCSK_LAUNCH) lifts him: PreDecide then finds him in the air (the air step 0xAF) and the
    // combo ends with the air finisher 0xB2 (MaxCombo 5 - Finishers 2 = 3 normal steps)
    assert_eq!(b.combo(), [0xAA, 0xAF, 0xAF, 0xB2], "the blades near one target");
    assert!(b.states(from).contains(&StateId::ComboSk));
    assert!(b.hp() < hp, "the blades hit");
    // ---- FINISH_SK: M_7419, the Chandelier (M_7423), its crash, M_1501; the form ends
    b.idle(10);
    let shots = b.sora.combat().shots_spawned;
    let from = b.log.len();
    let mut drawn = 0;
    b.step(&[PadButton::SITUATION]);
    let mut subs = Vec::new();
    for _ in 0..3000 {
        let out = b.step(&[]);
        if b.sora.xs_chandelier_pawn().is_some() {
            drawn += 1;
        }
        match b.sora.finish() {
            Some(f) if subs.last() != Some(&f.sub) => subs.push(f.sub),
            Some(_) => {}
            None if out.state != StateId::Stylefinish => break,
            None => {}
        }
    }
    assert_eq!(subs, [1, 2, 3, 6, 7, 8], "StyleFinishSK Tick's sub-steps (5 falls through to 6 in the tick the chandelier's and his M_7423 run out)");
    for c in ["M7419", "7423", "M1501"] {
        assert!(b.clips(from).iter().any(|x| x.replace('_', "") == c), "{c}: {:?}", b.clips(from));
    }
    assert!(drawn > 60, "the hosts are given the chandelier to draw ({drawn} steps)");
    assert!(b.sora.combat().shots_spawned > shots, "the chandelier's crash");
    b.idle(5);
    assert_eq!(b.sora.combat().form.cmd, 0);
    assert!(b.sora.combat().xs.chandelier.is_none());
}

/// (F-CLAWS) A wall between Sora and a far target: the claws' dashes (traction + scaled root motion, all in the air mode:
/// PhysFlying's swept move with its slide) and Anchor Shot's warp (slot 136: line tests from HIM; the port's swept placement)
/// never put him into or beyond it.
#[test]
fn a_wall_in_the_path_stops_the_dashes_and_the_warp() {
    let wall = (400.0, 450.0);
    let world = || TestWorld::floor().with_box(Vec3::new(wall.0, -1000.0, -10.0), Vec3::new(wall.1, 1000.0, 800.0));
    let radius = SORA_PARAMS.capsule_radius;
    // ---- Agile Claws and Blizzard Claws: the combo at a target behind the wall
    for (key, cmd) in [("WEP_KEYBLADE_SO_05", 0x8A), ("WEP_KEYBLADE_SO_06", 0x90)] {
        let mut b = Bench::new(key);
        b.transform(cmd);
        b.world = world();
        b.host.targets[0] = dummy(1, 700.0);
        b.idle(30);
        let mut far = f32::MIN;
        for k in 0..400 {
            let o = if k % 2 == 0 { b.step(&[PadButton::ATTACK]) } else { b.step(&[]) };
            far = far.max(o.position.x);
        }
        assert!(b.sora.combat().history.iter().any(|k| (0x88..=0x91).contains(k)), "{cmd:#X}: the claws attacked: {:?}", b.sora.combat().history);
        assert!(far <= wall.0 - radius + 1.0, "{cmd:#X}: he stays on his side of the wall (face {}, radius {radius}): got to x {far}", wall.0);
    }
    // (Anchor Shot's warp with a wall in its line: states/combo_cl.rs's unit test; the lock-on takes no target behind a wall,
    // so the move cannot be started at one here.)
}
