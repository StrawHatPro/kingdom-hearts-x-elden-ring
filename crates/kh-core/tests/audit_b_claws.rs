//! (AUDIT-B F-CLAWS) The claws package's behaviour items closed by the audit, through the real core
//! (build/sora_tmp/weapons/AUDIT-B/F-CLAWS/SECTION.md):
//! 1. FUN_1414145C0's second form switch (`case 0x80 0x82 0x83 0x85 0x88 0x8A 0x8B 0x90: return`): no guard-counter window in
//!    Agile Claws, Twin Yo-Yos, Blizzard Claws and Blizzard Blades, even with GUARD_COUNTER (the Keyblade's rule in Second Form).
//! 2. FINISH_YO (StyleFinish slot 149 0x1413E0CD0): the YO_Warhead window's YO_BIND hit is m_pCatchPawn; it is CARRIED on the
//!    giant yo-yo's X_link01 (`track.YO.4420.X_link01`) through M_4420 and given back at the end; his pitch is levelled.
//! 3. FINISH_SK (StyleFinishSK Tick 0x1413D6F30 sub 0): started in the air, he is put where the Chandelier stands (the floor).
//!    Traces: build/sora_tmp/weapons/AUDIT-B/F-CLAWS/traces/core_*.trace.

use kh_core::enums::ETresDamageKind;
use kh_core::states::HitOnPlayer;
use kh_core::status::LoadoutValue;
use kh_core::test_world::TestWorld;
use kh_core::{Input, Loadout, Output, PadButton, PadButtons, PadState, Pose, RecordingHost, Sora, StateId, Status, TargetFlags, TargetId, TargetInfo, Tuning, Vec3, SORA_PARAMS};

const DT: f32 = 1.0 / 60.0;

fn traces() -> std::path::PathBuf {
    let dir = std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../build/sora_tmp/weapons/AUDIT-B/F-CLAWS/traces"));
    std::fs::create_dir_all(&dir).ok();
    dir
}

fn cell(json: &serde_json::Value) -> Option<LoadoutValue> {
    match json {
        serde_json::Value::Number(n) => n.as_f64().map(LoadoutValue::Number),
        serde_json::Value::String(s) => Some(LoadoutValue::Text(s.clone())),
        serde_json::Value::Array(items) => items.iter().map(|i| i.as_str().map(str::to_string)).collect::<Option<Vec<_>>>().map(LoadoutValue::List),
        serde_json::Value::Object(map) => map.iter().map(|(k, v)| v.as_f64().map(|n| (k.clone(), n))).collect::<Option<Vec<_>>>().map(LoadoutValue::Map),
        _ => None,
    }
}

fn status(abilities: &[&str], weapon: &str) -> Status {
    let path = format!("{}/../../sheets/port/sora-loadout-99.json", env!("CARGO_MANIFEST_DIR"));
    let json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))).unwrap();
    let mut loadout = Loadout::new("sora-loadout-99");
    for row in json["rows"].as_array().unwrap() {
        let key = row[0].as_str().unwrap();
        let value = if key == "abilities.equipped" { LoadoutValue::List(abilities.iter().map(|a| a.to_string()).collect()) } else { cell(&row[1]).unwrap() };
        loadout.set(key, &value).unwrap_or_else(|e| panic!("{e}"));
    }
    loadout.resolve().unwrap();
    let mut s = Status::from_loadout(&loadout);
    s.weapon = weapon.to_string();
    s
}

fn dummy(at: Vec3) -> TargetInfo {
    TargetInfo { id: TargetId(1), position: at, center: at + Vec3::Z * 90.0, velocity: Vec3::ZERO, yaw: 180.0, radius: 40.0, half_height: 90.0, flags: TargetFlags(TargetFlags::ALIVE | TargetFlags::HITTABLE | TargetFlags::LOCKABLE | TargetFlags::ENEMY), hp: 1.0e6, hp_max: 1.0e6 }
}

struct Bench {
    sora: Sora,
    host: RecordingHost,
    world: TestWorld,
    held_before: Vec<PadButton>,
}

impl Bench {
    fn new(status: Status) -> Bench {
        let origin = Vec3::new(0.0, 0.0, Sora::origin_height_on_floor(&SORA_PARAMS));
        let sora = Sora::new(SORA_PARAMS, Pose { position: origin, yaw: 0.0 }).with_tuning(Tuning::GAME_EXACT).with_status(status);
        let mut host = RecordingHost::new();
        host.targets.push(dummy(Vec3::new(200.0, 0.0, 0.0)));
        let mut b = Bench { sora, host, world: TestWorld::floor(), held_before: Vec::new() };
        b.idle(5);
        b
    }

    fn step(&mut self, held: &[PadButton]) -> Output {
        let before = self.held_before.iter().fold(PadButtons::NONE, |all, b| all.with(*b));
        let input = Input::default().with_pad(PadState::from_held(held.iter().fold(PadButtons::NONE, |all, b| all.with(*b)), before));
        let (sora, world) = (&mut self.sora, &self.world);
        let out = self.host.step(world, |h| sora.step_hosted(DT, &input, h));
        self.held_before = held.to_vec();
        out
    }

    fn idle(&mut self, steps: usize) {
        for _ in 0..steps {
            self.step(&[]);
        }
    }

    fn change_to(&mut self, cmd: i32) -> bool {
        for _ in 0..4 {
            self.sora.debug_add_style(100.0);
            self.idle(15);
            for _ in 0..8 {
                if self.sora.combat().situation.current().is_some_and(|(_, k)| k == cmd) {
                    break;
                }
                self.step(&[PadButton::SITUATION_NEXT]);
                self.idle(1);
            }
            if !self.sora.combat().situation.current().is_some_and(|(_, k)| k == cmd) {
                continue;
            }
            self.step(&[PadButton::SITUATION]);
            for _ in 0..600 {
                if self.step(&[]).state != StateId::Stylechange {
                    break;
                }
            }
            self.idle(10);
            if self.sora.combat().form.cmd == cmd {
                return true;
            }
        }
        self.sora.combat().form.cmd == cmd
    }

    /// The guard, one guarded hit from the dummy in front: the window's (time, command) after it.
    fn guarded_hit(&mut self) -> (f32, i32) {
        let out = self.step(&[]);
        let (me, fwd) = (out.position, Vec3::new(out.yaw.to_radians().cos(), out.yaw.to_radians().sin(), 0.0));
        let at = Vec3::new(me.x, me.y, 0.0) + fwd * 200.0;
        self.host.targets[0] = dummy(at);
        self.idle(20);
        for _ in 0..10 {
            self.step(&[PadButton::GUARD]);
        }
        let d = (Vec3::new(me.x, me.y, 0.0) - at).normalize_or_zero();
        self.sora.take_hit(HitOnPlayer { direction: d, causer_yaw: d.y.atan2(d.x).to_degrees(), ..HitOnPlayer::new(ETresDamageKind::Small, 30.0, TargetId(1), at) });
        self.step(&[PadButton::GUARD]);
        (self.sora.actions().counter_window, self.sora.actions().counter_command)
    }
}

#[test]
fn no_guard_counter_window_in_the_four_forms() {
    let abilities = ["REFLECT_GUARD", "GUARD_COUNTER"];
    let mut log = String::new();
    // the control: the Keyblade (no form) opens Counter Slash's window
    let mut b = Bench::new(status(&abilities, "WEP_KEYBLADE_SO_05"));
    let (w, c) = b.guarded_hit();
    log.push_str(&format!("no form: window {w:.2} command {c}\n"));
    assert!(w > 0.0 && c == 3, "the Keyblade's rule opens the window: {w} {c}");
    for (weapon, first, form) in [("WEP_KEYBLADE_SO_05", 0, 0x8A), ("WEP_KEYBLADE_SO_05", 0x8A, 0x82), ("WEP_KEYBLADE_SO_06", 0, 0x90), ("WEP_KEYBLADE_SO_06", 0x90, 0x85)] {
        let mut b = Bench::new(status(&abilities, weapon));
        if first != 0 {
            assert!(b.change_to(first), "{first:#X}");
        }
        assert!(b.change_to(form), "{form:#X}");
        let (w, c) = b.guarded_hit();
        log.push_str(&format!("form {form:#X}: window {w:.2} command {c}\n"));
        assert!(w <= 0.0 && c == 0, "form {form:#X}: FUN_1414145C0 returns before opening a window ({w} {c})");
    }
    std::fs::write(traces().join("core_guard_counter.trace"), log).ok();
}

#[test]
fn finish_yo_carries_its_catch_on_x_link01() {
    let mut b = Bench::new(status(&[], "WEP_KEYBLADE_SO_05"));
    assert!(b.change_to(0x8A));
    assert!(b.change_to(0x82));
    // the dummy right in front, two full gauges: FINISH_YO in the finish slot
    let out = b.step(&[]);
    let fwd = Vec3::new(out.yaw.to_radians().cos(), out.yaw.to_radians().sin(), 0.0);
    b.host.targets[0] = dummy(Vec3::new(out.position.x, out.position.y, 0.0) + fwd * 250.0);
    b.idle(10);
    for _ in 0..30 {
        if b.sora.combat().situation.slot_b.map(|e| e.kind) == Some(0x9C) {
            break;
        }
        b.sora.debug_add_style(100.0);
        b.idle(10);
    }
    assert_eq!(b.sora.combat().situation.slot_b.map(|e| e.kind), Some(0x9C), "FINISH_YO on offer");
    let start = b.host.targets[0].position;
    b.step(&[PadButton::SITUATION]);
    let mut log = String::new();
    let (mut subs, mut carried, mut far) = (Vec::new(), 0, 0.0f32);
    for _ in 0..1200 {
        let out = b.step(&[]);
        if let Some(f) = b.sora.finish() {
            if subs.last() != Some(&f.sub) {
                subs.push(f.sub);
            }
        }
        let held = b.host.holds.last().copied();
        if let Some((_, Some(h))) = held {
            if h.carry.is_some() {
                carried += 1;
                far = far.max((b.host.targets[0].position - start).length());
            }
        }
        log.push_str(&format!("{:?} {} t {:.3} pos {:.1?} pitch {:.2} dummy {:.1?} hold {:?}\n", out.state, out.anim.name.name(), out.anim.time, out.position, out.pitch, b.host.targets[0].position, held.map(|h| h.1.and_then(|x| x.carry))));
        if out.state != StateId::Stylefinish && b.sora.combat().form.cmd == 0 {
            break;
        }
    }
    std::fs::write(traces().join("core_finish_yo_catch.trace"), log).ok();
    assert!(subs.contains(&5), "{subs:?}");
    assert!(carried > 30, "the caught pawn rides on X_link01 ({carried} steps)");
    assert!(far > 100.0, "X_link01 moves it with the giant yo-yo ({far} cm)");
    assert!(matches!(b.host.holds.last(), Some((TargetId(1), None))), "given back at the end: {:?}", b.host.holds.last());
    assert!(b.sora.output().pitch.abs() < 1.0e-3, "his pitch levelled");
}

#[test]
fn finish_sk_in_the_air_puts_him_where_the_chandelier_stands() {
    let mut b = Bench::new(status(&[], "WEP_KEYBLADE_SO_06"));
    assert!(b.change_to(0x90));
    assert!(b.change_to(0x85));
    for _ in 0..30 {
        if b.sora.combat().situation.slot_b.map(|e| e.kind) == Some(0x9F) {
            break;
        }
        b.sora.debug_add_style(100.0);
        b.idle(10);
    }
    assert_eq!(b.sora.combat().situation.slot_b.map(|e| e.kind), Some(0x9F), "FINISH_SK on offer");
    // a jump: in the air when Triangle is pressed
    b.step(&[PadButton::JUMP]);
    let mut high = 0.0f32;
    for _ in 0..12 {
        high = high.max(b.step(&[PadButton::JUMP]).position.z);
    }
    let floor = Sora::origin_height_on_floor(&SORA_PARAMS);
    assert!(b.sora.output().position.z > floor + 50.0, "in the air ({})", b.sora.output().position.z);
    b.step(&[PadButton::SITUATION]);
    let mut log = String::new();
    let mut placed = None;
    for _ in 0..30 {
        let out = b.step(&[]);
        log.push_str(&format!("{:?} {} sub {:?} z {:.2}\n", out.state, out.anim.name.name(), b.sora.finish().map(|f| f.sub), out.position.z));
        if b.sora.finish().is_some_and(|f| f.sub >= 1) && placed.is_none() {
            placed = Some(out.position.z);
        }
    }
    std::fs::write(traces().join("core_finish_sk_place.trace"), log).ok();
    let z = placed.expect("FINISH_SK began");
    assert!((z - floor).abs() < 2.0, "on the floor under him, where the Chandelier landed (z {z}, floor origin {floor}, jump top {high})");
}
