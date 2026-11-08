//! (INT-F, the coordinator's decision 2026-11-03) THE GUARD COUNTER IS SHOWN ON ATTACK ONLY, KH3 1:1: KH3 labels the Attack
//! slot with the counter window's command (0x1414145C0 -> 0x14158C180 -> 0x14162A440 -> setMainCommandText(0, name)) and
//! never lists it as a situation command. Before the port's situation-slot copy of the window (kh-core `combat/guard.rs`
//! `mirror_prompts`, its counter-window half) could go, this proves that PRESSING ATTACK IN THE WINDOW triggers every guard
//! counter: the Keyblade's Revenge kinds (Counter Slash 0x21, Revenge Impact 0x23, Revenge Dive 0x24) and every form whose
//! window opens (FUN_1414145C0's form table, `actions::form_counter`): their forced kind 0x25 (Barrier Cracker / Nano
//! Counter / Ultimate Form's counter) or the shields' grown attack (0x55 / 0x56).
//!
//! The session per case: the form entered from a full gauge (two stages where the form is a second stage), the dummy 2 m in
//! front, the guard held, one guarded hit (the window opens: its command checked against KH3's table), then ATTACK only.

use kh_core::enums::ETresDamageKind;
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

/// The level-99 loadout with these abilities equipped, holding `weapon`.
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

    /// A full gauge, the offer, Triangle, through the transformation; true when `cmd` is held after it.
    fn change_to(&mut self, cmd: i32) -> bool {
        for _ in 0..4 {
            self.sora.debug_add_style(100.0);
            self.idle(15);
            // the offer of `cmd` selected (L2 cycles through the entries)
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
}

/// What one guard-counter session gave: the window's command, then the attack kind ATTACK started (None: no attack).
#[derive(Debug, PartialEq)]
struct Counter {
    form: i32,
    command: i32,
    kind: Option<u8>,
    window_after: f32,
    seen: Vec<String>,
}

/// The guard, one guarded hit from the dummy in front, then ATTACK (pressed until an attack starts, at most 0.5 s).
fn counter_with_attack(b: &mut Bench) -> Counter {
    let button = PadButton::ATTACK;
    let out = b.step(&[]);
    let (me, fwd) = (out.position, Vec3::new(out.yaw.to_radians().cos(), out.yaw.to_radians().sin(), 0.0));
    let at = Vec3::new(me.x, me.y, 0.0) + fwd * 200.0;
    b.host.targets[0] = dummy(at);
    b.idle(20);
    for _ in 0..10 {
        b.step(&[PadButton::GUARD]);
    }
    let d = (Vec3::new(me.x, me.y, 0.0) - at).normalize_or_zero();
    b.sora.take_hit(HitOnPlayer { direction: d, causer_yaw: d.y.atan2(d.x).to_degrees(), ..HitOnPlayer::new(ETresDamageKind::Small, 30.0, TargetId(1), at) });
    b.step(&[PadButton::GUARD]);
    let (form, command, window) = (b.sora.combat().form.cmd, b.sora.actions().counter_command, b.sora.actions().counter_window);
    if window <= 0.0 {
        // no window in this form: no counter to trigger
        return Counter { form, command: 0, kind: None, window_after: 0.0, seen: vec!["no window".to_string()] };
    }
    b.step(&[]);
    let mut kind = None;
    let mut seen = Vec::new();
    for k in 0..30 {
        // a press each other step (a held button is not a new press)
        let held: &[PadButton] = if k % 2 == 0 { &[button] } else { &[] };
        let out = b.step(held);
        let clip = out.anim.name.name();
        seen.push(format!("{:?}:{clip}", out.state));
        if let Some(a) = b.sora.attack() {
            kind = Some(a.kind);
            break;
        }
        // COMBO_GM / COMBO_SH are not in `Sora::attack()`'s view: their counter by the clip the kind plays (nano.rs
        // `attack_row(COUNTER)` = MagicCast / MagicAirCast; shield.rs 0x55 = M_B418, 0x56 = M_B426)
        kind = match (out.state, clip) {
            (StateId::ComboGm, "MagicCast" | "MagicAirCast") => Some(0x25),
            (StateId::ComboSh, "B418") => Some(0x55),
            (StateId::ComboSh, "B426") => Some(0x56),
            _ => None,
        };
        if kind.is_some() {
            break;
        }
    }
    seen.dedup();
    if form == 0x86 && kind.is_some() {
        // the Nano Counter's shot (BARRIERCRACKER) is fired
        for _ in 0..120 {
            b.step(&[]);
        }
        assert!(b.sora.combat().nano.counters_fired >= 1, "the Nano Counter fires its BARRIERCRACKER");
    }
    Counter { form, command, kind, window_after: b.sora.actions().counter_window, seen }
}

/// (KH3's table, `actions::form_counter`) the form -> its window's command, and the kinds a counter of it may start.
fn expected(form: i32, abilities: &[&str]) -> (i32, &'static [u8]) {
    match form {
        0x86 => (0x0E, &[0x25]),
        0x87 => (0x0F, &[0x25]),
        0x89 => (0x10, &[0x55, 0x56]),
        0x91 => (0x12, &[0x55, 0x56]),
        0x8E | 0x8F | 0x92 | 0x93 => (6, &[0x25]),
        0xDD..=0xE0 => (0, &[0x25]),
        _ if abilities.contains(&"REVENGEDIVE") => (5, &[0x24]),
        _ if abilities.contains(&"REVENGEIMPACT") => (4, &[0x23]),
        _ => (3, &[0x21]),
    }
}

/// (form command or 0, the Keyblade, its first stage when the form is a second stage)
const CASES: &[(i32, &str, i32)] = &[
    (0, "WEP_KEYBLADE_SO_00", 0),
    (0x94, "WEP_KEYBLADE_SO_00", 0),
    (0x86, "WEP_KEYBLADE_SO_08", 0),
    (0x87, "WEP_KEYBLADE_SO_14", 0),
    (0x89, "WEP_KEYBLADE_SO_01", 0),
    (0x91, "WEP_KEYBLADE_SO_10", 0),
    (0x8E, "WEP_KEYBLADE_SO_07", 0),
    (0x8F, "WEP_KEYBLADE_SO_07", 0x8E),
    (0x92, "WEP_KEYBLADE_SO_11", 0),
    (0x93, "WEP_KEYBLADE_SO_11", 0x92),
    (0xDD, "WEP_KEYBLADE_SO_12", 0),
    (0xDE, "WEP_KEYBLADE_SO_13", 0),
    (0xDF, "WEP_KEYBLADE_SO_12", 0xDD),
    (0xE0, "WEP_KEYBLADE_SO_13", 0xDE),
    // the forms FUN_1414145C0 gives no window of their own (the Keyblade's rule, with GUARD_COUNTER)
    (0x95, "WEP_KEYBLADE_SO_17", 0),
    (0x98, "WEP_KEYBLADE_SO_18", 0),
    (0x80, "WEP_KEYBLADE_SO_02", 0),
    (0x83, "WEP_KEYBLADE_SO_02", 0x80),
    (0x81, "WEP_KEYBLADE_SO_03", 0),
    (0x84, "WEP_KEYBLADE_SO_03", 0x81),
    (0x88, "WEP_KEYBLADE_SO_04", 0),
    (0x8A, "WEP_KEYBLADE_SO_05", 0),
    (0x82, "WEP_KEYBLADE_SO_05", 0x8A),
    (0x90, "WEP_KEYBLADE_SO_06", 0),
    (0x85, "WEP_KEYBLADE_SO_06", 0x90),
    (0x8C, "WEP_KEYBLADE_SO_09", 0),
    (0x8D, "WEP_KEYBLADE_SO_09", 0x8C),
];

#[test]
fn attack_in_the_counter_window_triggers_every_guard_counter() {
    let mut report = Vec::new();
    let mut failed = Vec::new();
    // The Keyblade's three Revenge kinds (no form).
    for abilities in [&["REFLECT_GUARD", "GUARD_COUNTER"][..], &["REFLECT_GUARD", "REVENGEIMPACT"], &["REFLECT_GUARD", "REVENGEDIVE"]] {
        let mut b = Bench::new(status(abilities, "WEP_KEYBLADE_SO_00"));
        let c = counter_with_attack(&mut b);
        let (command, kinds) = expected(0, abilities);
        let ok = c.command == command && c.kind.is_some_and(|k| kinds.contains(&k));
        report.push(format!("no form {abilities:?}: {c:?} (KH3: command {command}, kinds {kinds:X?}) {}", if ok { "ok" } else { "FAILED" }));
        if !ok {
            failed.push(report.last().cloned().unwrap_or_default());
        }
    }
    // Every form whose window opens, and Second Form (the Keyblade's rule inside a form).
    let abilities = ["REFLECT_GUARD", "GUARD_COUNTER"];
    for (form, weapon, first) in CASES.iter().copied().filter(|c| c.0 != 0) {
        let mut b = Bench::new(status(&abilities, weapon));
        if first != 0 {
            assert!(b.change_to(first), "{weapon}: its first stage {first:#X} (form {:#X})", b.sora.combat().form.cmd);
        }
        assert!(b.change_to(form), "{weapon}: the form {form:#X} (form {:#X})", b.sora.combat().form.cmd);
        let c = counter_with_attack(&mut b);
        let (command, kinds) = expected(form, &abilities);
        let command_ok = command == 0 || c.command == command;
        let ok = c.seen == ["no window"] || (command_ok && c.kind.is_some_and(|k| kinds.contains(&k)));
        report.push(format!("form {form:#X} ({weapon}): {c:?} (KH3: command {command:#X}, kinds {kinds:X?}) {}", if ok { "ok" } else { "FAILED" }));
        if !ok {
            failed.push(report.last().cloned().unwrap_or_default());
        }
    }
    for line in &report {
        eprintln!("{line}");
    }
    assert!(failed.is_empty(), "guard counters ATTACK does not trigger:\n{}", failed.join("\n"));
}
