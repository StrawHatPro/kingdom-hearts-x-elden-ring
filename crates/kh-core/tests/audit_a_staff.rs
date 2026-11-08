//! (AUDIT-A STAFF) Mirage Staff (Ever After, `form.WA` CHANGE_WA 0x88): the core traces of the AUDIT-A STAFF fixes
//! (build/sora_tmp/weapons/AUDIT-A/STAFF.md), written to build/sora_tmp/weapons/AUDIT-A/logs/staff_core.trace:
//! - the camera blueprint's events (AUDIT-C's `states/bp_event.rs` path): A401's BpEvent in STYLECHANGE, A417's in FINISH_WA,
//!   the specific actions of StyleFinishWA (Tick sub 0 pawn vt+0x1038(0, 0); the cleanup 0x1413CFE90 vt+0x1038(0x41));
//! - the world slow of FINISH_WA held until the tower's first attack window (its +0xB48: the anim interface's +0x200 at
//!   FinishWA's NotifyBegin, 6.2 s of its A417), the tower's hits after it, M_1501 and the end;
//! - invincibility in STYLECHANGE and FINISH_WA (`Status::damage_invalid`);
//! - no guard-counter window in Mirage Staff (FUN_1414145C0's second switch `case 0x88: return`), against the same guarded
//!   hit with the Kingdom Key.

use kh_core::enums::ETresDamageKind;
use kh_core::host::EventKind;
use kh_core::states::HitOnPlayer;
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

fn holding(key: &str, abilities: &[&str]) -> Status {
    let path = format!("{}/../../sheets/port/sora-loadout-99.json", env!("CARGO_MANIFEST_DIR"));
    let json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))).unwrap();
    let mut loadout = Loadout::new("sora-loadout-99");
    for row in json["rows"].as_array().unwrap() {
        let k = row[0].as_str().unwrap();
        let value = if k == "abilities.equipped" { LoadoutValue::List(abilities.iter().map(|a| a.to_string()).collect()) } else { cell(&row[1]).unwrap() };
        loadout.set(k, &value).unwrap_or_else(|e| panic!("{e}"));
    }
    loadout.resolve().unwrap();
    let mut s = Status::from_loadout(&loadout);
    s.weapon = key.to_string();
    s
}

fn dummy(x: f32) -> TargetInfo {
    TargetInfo { id: TargetId(1), position: Vec3::new(x, 0.0, 0.0), center: Vec3::new(x, 0.0, 90.0), velocity: Vec3::ZERO, yaw: 180.0, radius: 40.0, half_height: 90.0, flags: TargetFlags(TargetFlags::ALIVE | TargetFlags::HITTABLE | TargetFlags::LOCKABLE | TargetFlags::ENEMY), hp: 1.0e9, hp_max: 1.0e9 }
}

struct Bench {
    sora: Sora,
    host: RecordingHost,
    world: TestWorld,
    step: u32,
    trace: Vec<String>,
}

impl Bench {
    fn new(key: &str, abilities: &[&str], at: f32) -> Bench {
        let origin = Vec3::new(0.0, 0.0, Sora::origin_height_on_floor(&SORA_PARAMS));
        let sora = Sora::new(SORA_PARAMS, Pose { position: origin, yaw: 0.0 }).with_tuning(Tuning::GAME_EXACT).with_status(holding(key, abilities));
        let mut host = RecordingHost::new();
        host.targets.push(dummy(at));
        let mut b = Bench { sora, host, world: TestWorld::floor(), step: 0, trace: Vec::new() };
        for _ in 0..5 {
            b.press(&[]);
        }
        b
    }

    /// One step; the events this trace follows are written with Sora's state, clip and the tower's time.
    fn press(&mut self, held: &[PadButton]) -> Output {
        let set = held.iter().fold(PadButtons::NONE, |all, b| all.with(*b));
        let input = Input::default().with_pad(PadState::from_held(set, PadButtons::NONE));
        let (sora, world) = (&mut self.sora, &self.world);
        let out = self.host.step(world, |h| sora.step_hosted(DT, &input, h));
        self.step += 1;
        let clip = out.anim_ticked.body_samples()[0];
        let tower = kh_core::combat::forms::staff::tower(self.sora.finish(), self.sora.combat()).map_or(String::from("-"), |t| format!("{:.3}", t.time));
        for e in &out.events {
            let keep = matches!(e.kind, EventKind::AnimBpEvent { .. } | EventKind::PlayerSpecificAction { .. } | EventKind::TimeDilation { .. } | EventKind::StateChange { .. });
            if keep {
                self.trace.push(format!("step {:5} {:?} sub {} {}@{:.3} tower {} invincible {} | {:?}", self.step, out.state, out.sub_step, clip.0, clip.2, tower, self.sora.status().damage_invalid, e.kind));
            }
        }
        out
    }

    fn idle(&mut self, n: usize) {
        for _ in 0..n {
            self.press(&[]);
        }
    }

    fn write(&self, name: &str) {
        let dir = format!("{}/../../build/sora_tmp/weapons/AUDIT-A/logs", env!("CARGO_MANIFEST_DIR"));
        std::fs::create_dir_all(&dir).ok();
        std::fs::write(format!("{dir}/{name}"), self.trace.join("\n") + "\n").ok();
    }
}

fn find(trace: &[String], what: &str) -> Option<usize> {
    trace.iter().position(|l| l.contains(what))
}

#[test]
fn mirage_staff_camera_events_world_slow_and_invincibility() {
    let mut b = Bench::new("WEP_KEYBLADE_SO_04", &[], 200.0);
    b.sora.debug_add_style(100.0);
    b.idle(15);
    assert_eq!(b.sora.combat().situation.list.first().map(|e| e.kind), Some(0x88), "CHANGE_WA offered");
    b.trace.push("# offer: Triangle on CHANGE_WA".into());
    b.press(&[PadButton::SITUATION]);
    let mut invincible_in_change = false;
    for _ in 0..600 {
        let out = b.press(&[]);
        invincible_in_change |= out.state == StateId::Stylechange && b.sora.status().damage_invalid;
        if out.state != StateId::Stylechange && b.sora.combat().form.cmd == 0x88 {
            break;
        }
    }
    assert!(invincible_in_change, "invincible through STYLECHANGE");
    let a401 = find(&b.trace, "AnimBpEvent { seq: \"A401\", param: 0, begin: true }").expect("A401's BpEvent starts (Rapunzel_key's transformation camera)");
    assert!(b.trace[a401].contains("Stylechange"), "{}", b.trace[a401]);
    assert!(find(&b.trace, "AnimBpEvent { seq: \"A401\", param: 0, begin: false }").is_some(), "and ends");
    b.idle(30);
    // ---- FINISH_WA
    b.trace.push("# FINISH_WA: Triangle".into());
    let from = b.trace.len();
    b.press(&[PadButton::SITUATION]);
    let (mut slow_off_at, mut tower_max, mut invincible_all) = (None, 0.0f32, true);
    for _ in 0..1500 {
        let out = b.press(&[]);
        if out.state == StateId::Stylefinish {
            invincible_all &= b.sora.status().damage_invalid;
        }
        if let Some(t) = kh_core::combat::forms::staff::tower(b.sora.finish(), b.sora.combat()) {
            tower_max = tower_max.max(t.time);
        }
        if slow_off_at.is_none() && out.events.iter().any(|e| matches!(e.kind, EventKind::TimeDilation { rate, .. } if (rate - 1.0).abs() < 1.0e-6)) {
            slow_off_at = kh_core::combat::forms::staff::tower(b.sora.finish(), b.sora.combat()).map(|t| t.time);
        }
        if out.state != StateId::Stylefinish && b.sora.combat().form.cmd == 0 {
            break;
        }
    }
    b.idle(10);
    b.write("staff_core.trace");
    let t = &b.trace[from..];
    assert!(invincible_all, "invincible through FINISH_WA");
    let spawn = find(t, "PlayerSpecificAction { state: StylefinishWa, action: 0, sub: 0 }").expect("Tick sub 0's pawn vt+0x1038(0, 0)");
    let begin = find(t, "AnimBpEvent { seq: \"A417\", param: 0, begin: true }").expect("A417's BpEvent (FINISH_WA's camera)");
    assert!(spawn <= begin + 1, "the specific action with the tower, before / with A417's start: {} / {}", t[spawn], t[begin]);
    let slow = slow_off_at.expect("the world slow ends during the tower");
    assert!((slow - 6.2).abs() < 2.0 * DT, "the world slow ends at the tower's first attack window (6.2 s), not its first tick: {slow}");
    let cleanup = find(t, "PlayerSpecificAction { state: StylefinishWa, action: 65, sub: 0 }").expect("the cleanup's vt+0x1038(0x41)");
    assert_eq!(t.iter().filter(|l| l.contains("action: 65")).count(), 1, "once (sub 6; End finds the tower gone)");
    assert!(t[cleanup].contains("sub 6") || t[cleanup].contains("sub 7"), "{}", t[cleanup]);
    assert!(find(t, "AnimBpEvent { seq: \"A417\", param: 0, begin: false }").is_some(), "A417's BpEvent ends");
    assert!(tower_max > 9.0, "the tower ran its A417: {tower_max}");
    assert_eq!(b.sora.combat().form.cmd, 0, "the finish ends the form");
    assert!(!b.sora.status().damage_invalid, "vulnerable again after the finish");
}

/// A guarded hit with GUARD_COUNTER: the Kingdom Key opens the window (3); in Mirage Staff (FUN_1414145C0's second switch,
/// `case 0x88: return`) none opens, whatever the abilities (the battle flag off, so the guard button guards).
#[test]
fn no_guard_counter_window_in_mirage_staff() {
    let guarded_hit = |b: &mut Bench| {
        let out = b.press(&[]);
        let (me, fwd) = (out.position, Vec3::new(out.yaw.to_radians().cos(), out.yaw.to_radians().sin(), 0.0));
        let at = Vec3::new(me.x, me.y, 0.0) + fwd * 200.0;
        b.host.targets[0] = TargetInfo { position: at, center: at + Vec3::Z * 90.0, ..dummy(0.0) };
        b.idle(20);
        b.press(&[PadButton::GUARD]);
        b.idle(6);
        let state = b.press(&[]).state;
        let d = (Vec3::new(me.x, me.y, 0.0) - at).normalize_or_zero();
        b.sora.take_hit(HitOnPlayer { direction: d, causer_yaw: d.y.atan2(d.x).to_degrees(), ..HitOnPlayer::new(ETresDamageKind::Small, 30.0, TargetId(1), at) });
        b.idle(1);
        (state, b.sora.actions().counter_window)
    };
    let abilities = ["REFLECT_GUARD", "GUARD_COUNTER"];
    let mut kk = Bench::new("WEP_KEYBLADE_SO_00", &abilities, 400.0);
    let (state, window) = guarded_hit(&mut kk);
    assert_eq!(state, StateId::Guard, "the Kingdom Key guards");
    assert!(window > 0.0, "the Kingdom Key's counter window");
    let mut b = Bench::new("WEP_KEYBLADE_SO_04", &abilities, 400.0);
    b.sora.debug_add_style(100.0);
    b.idle(15);
    b.press(&[PadButton::SITUATION]);
    for _ in 0..600 {
        if b.press(&[]).state != StateId::Stylechange && b.sora.combat().form.cmd == 0x88 {
            break;
        }
    }
    b.idle(20);
    assert_eq!(b.sora.combat().form.cmd, 0x88);
    let (state, window) = guarded_hit(&mut b);
    b.trace.push(format!("# Mirage Staff: guard press -> {state:?}, a guarded hit with GUARD_COUNTER -> counter window {window}"));
    eprintln!("Mirage Staff: guard press -> {state:?}, counter window {window}");
    assert_eq!(window, 0.0, "no guard-counter window in Mirage Staff ({state:?})");
}
