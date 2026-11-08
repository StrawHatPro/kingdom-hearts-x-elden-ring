//! (REVENGE, 2026-11-07) THE HOST-ASKED COUNTER (`Sora::request_counter`, abilities/xs/counter.rs): the Mimic Tear boss
//! copies' answer to a full revenge value. Without a press of the player's: Sonic Blade's dash and launch (XS 0xE0 /
//! 0xE1, KH3's LM_SONICRAVE clips M_1425 / M_1426) then Zantetsuken (XS 0xE4, M_M411) with the Keyblade; a form's own move
//! (Hyper Hammer's Giant Stamp 0x5D, M_3416) through its own attack class. Nothing happens without an ask.

use kh_core::status::LoadoutValue;
use kh_core::test_world::TestWorld;
use kh_core::{Input, Loadout, PadButton, PadButtons, PadState, Pose, RecordingHost, Sora, StateId, Status, TargetFlags, TargetId, TargetInfo, Tuning, Vec3, SORA_PARAMS};

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

/// The level-99 loadout holding Keyblade `key`, no abilities equipped (no XS ability is needed for a counter).
fn holding(key: &str) -> Status {
    let path = format!("{}/../../sheets/port/sora-loadout-99.json", env!("CARGO_MANIFEST_DIR"));
    let json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let mut loadout = Loadout::new("sora-loadout-99");
    for row in json["rows"].as_array().unwrap() {
        let k = row[0].as_str().unwrap();
        let value = if k == "abilities.equipped" { LoadoutValue::List(Vec::new()) } else { cell(&row[1]).unwrap() };
        loadout.set(k, &value).unwrap();
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
}

impl Bench {
    fn new(key: &str) -> Bench {
        let origin = Vec3::new(0.0, 0.0, Sora::origin_height_on_floor(&SORA_PARAMS));
        let sora = Sora::new(SORA_PARAMS, Pose { position: origin, yaw: 0.0 }).with_tuning(Tuning::GAME_EXACT).with_status(holding(key));
        let mut host = RecordingHost::new();
        host.targets.push(dummy(1, 250.0));
        let mut b = Bench { sora, host, world: TestWorld::floor() };
        b.idle(5);
        b
    }

    fn step(&mut self, held: &[PadButton]) -> StateId {
        let set = held.iter().fold(PadButtons::NONE, |all, b| all.with(*b));
        let input = Input::default().with_pad(PadState::from_held(set, PadButtons::NONE));
        let (sora, world) = (&mut self.sora, &self.world);
        self.host.step(world, |h| sora.step_hosted(DT, &input, h)).state
    }

    fn idle(&mut self, n: usize) {
        for _ in 0..n {
            self.step(&[]);
        }
    }

    /// The formchange `cmd` offered and taken (form_hammer.rs's way).
    fn transform(&mut self, cmd: i32) {
        self.sora.debug_add_style(100.0);
        self.idle(15);
        assert!(self.sora.combat().situation.list.iter().any(|e| e.kind == cmd), "the formchange {cmd:#x} is offered");
        self.step(&[PadButton::SITUATION]);
        for _ in 0..600 {
            if self.step(&[]) != StateId::Stylechange {
                return;
            }
        }
        panic!("STYLECHANGE did not end");
    }

    fn names(&self) -> Vec<&'static str> {
        self.host.hits().into_iter().map(|h| h.0.attack_name).collect()
    }
}

/// Positions of `want` in order within `history`.
fn in_order(history: &[u8], want: &[u8]) -> bool {
    let mut it = history.iter();
    want.iter().all(|w| it.any(|k| k == w))
}

#[test]
fn revenge_a_keyblade_counter_is_sonic_blade_then_zantetsuken_without_a_press() {
    let mut b = Bench::new("WEP_KEYBLADE_SO_00");
    let before = b.sora.combat().history.len();
    b.sora.request_counter(&[0xE0, 0xE1, 0xE4], TargetId(1), false);
    assert_eq!(b.sora.counter_state(), Some((3, 0)));
    for _ in 0..900 {
        b.step(&[]);
        if b.sora.counter_state().is_none() {
            break;
        }
    }
    b.idle(120);
    let played = b.sora.combat().history[before..].to_vec();
    assert!(in_order(&played, &[0xE0, 0xE1, 0xE4]), "Sonic Blade's dash, its launch, then Zantetsuken: {played:x?}");
    assert_eq!(b.sora.counter_state(), None, "the counter is over");
    let names = b.names();
    assert!(names.iter().any(|n| n.starts_with("SCLF_ZANTETSUKEN")), "Zantetsuken's row hit: {names:?}");
}

#[test]
fn revenge_a_hyper_hammer_counter_is_its_own_giant_stamp() {
    let mut b = Bench::new("WEP_KEYBLADE_SO_03");
    b.transform(0x81);
    assert_eq!(b.sora.combat().form.cmd, 0x81);
    let before = b.sora.combat().history.len();
    b.sora.request_counter(&[0x5D], TargetId(1), true);
    for _ in 0..600 {
        b.step(&[]);
        if b.sora.counter_state().is_none() {
            break;
        }
    }
    b.idle(240);
    let played = b.sora.combat().history[before..].to_vec();
    assert_eq!(played.first(), Some(&0x5D), "the hammer's own Giant Stamp: {played:x?}");
    assert!(b.names().iter().any(|n| n.starts_with("SCHM_GIANTSTAMP")), "{:?}", b.names());
}

/// The counter of every form whose weapon is not a Keyblade (kh-er mimicrevenge.rs `sora_counter`): (its Keyblade, the
/// form commands taken to reach it, the form's own kind, the row its hit carries).
const FORM_COUNTERS: [(&str, &[i32], u8, &str); 18] = [
    ("WEP_KEYBLADE_SO_01", &[0x89], 0x52, "SCSH_RAID"),
    ("WEP_KEYBLADE_SO_10", &[0x91], 0x52, "SH2_RAID"),
    ("WEP_KEYBLADE_SO_02", &[0x80], 0x47, ""),
    ("WEP_KEYBLADE_SO_07", &[0x8E], 0x47, ""),
    ("WEP_KEYBLADE_SO_02", &[0x80, 0x83], 0x4C, ""),
    ("WEP_KEYBLADE_SO_07", &[0x8E, 0x8F], 0x4C, ""),
    ("WEP_KEYBLADE_SO_03", &[0x81], 0x5D, "SCHM_GIANTSTAMP"),
    ("WEP_KEYBLADE_SO_11", &[0x92], 0x5D, ""),
    ("WEP_KEYBLADE_SO_03", &[0x81, 0x84], 0x6A, "SCDR_GIGAUPPER"),
    ("WEP_KEYBLADE_SO_11", &[0x92, 0x93], 0x6A, ""),
    ("WEP_KEYBLADE_SO_04", &[0x88], 0x83, "SCWA_BREAK"),
    ("WEP_KEYBLADE_SO_05", &[0x8A], 0x89, "SCCL_STAB"),
    ("WEP_KEYBLADE_SO_06", &[0x90], 0x89, ""),
    ("WEP_KEYBLADE_SO_05", &[0x8A, 0x82], 0x9A, "SCYO_WALKTHEDOG"),
    ("WEP_KEYBLADE_SO_06", &[0x90, 0x85], 0xAB, "SCSK_RUSH"),
    ("WEP_KEYBLADE_SO_08", &[0x86], 0xB5, "SCGM_CRASH"),
    ("WEP_KEYBLADE_SO_09", &[0x8C], 0x9F, "SCHL_THRUST"),
    ("WEP_KEYBLADE_SO_09", &[0x8C, 0x8D], 0xA7, "SCFL_LAUNCH"),
];

#[test]
fn revenge_every_form_counters_with_its_own_move() {
    let mut failed = Vec::new();
    for (key, forms, kind, row) in FORM_COUNTERS {
        let mut b = Bench::new(key);
        for f in forms {
            b.transform(*f);
        }
        let cmd = *forms.last().unwrap();
        assert_eq!(b.sora.combat().form.cmd, cmd, "{key}: the form {cmd:#x}");
        let before = b.sora.combat().history.len();
        b.sora.request_counter(&[kind], TargetId(1), true);
        for _ in 0..600 {
            b.step(&[]);
            if b.sora.counter_state().is_none() {
                break;
            }
        }
        b.idle(240);
        let played = b.sora.combat().history[before..].to_vec();
        let hit = row.is_empty() || b.names().iter().any(|n| n.starts_with(row));
        if played.first() != Some(&kind) || !hit {
            failed.push(format!("{key} form {cmd:#x}: wanted {kind:#x} ({row}), played {played:x?}, hit {hit}"));
        }
    }
    assert!(failed.is_empty(), "{failed:#?}");
}

/// The blade forms play Sonic Blade and Zantetsuken through the plain attack state (their own request skipped).
#[test]
fn revenge_the_blade_forms_counter_with_sonic_blade_then_zantetsuken() {
    let mut failed = Vec::new();
    for (key, cmd) in [("WEP_KEYBLADE_SO_00", 0x94), ("WEP_KEYBLADE_SO_12", 0xDD), ("WEP_KEYBLADE_SO_13", 0xDE), ("WEP_KEYBLADE_SO_14", 0x87)] {
        let mut b = Bench::new(key);
        b.transform(cmd);
        let before = b.sora.combat().history.len();
        b.sora.request_counter(&[0xE0, 0xE1, 0xE4], TargetId(1), false);
        for _ in 0..900 {
            b.step(&[]);
            if b.sora.counter_state().is_none() {
                break;
            }
        }
        b.idle(120);
        let played = b.sora.combat().history[before..].to_vec();
        if !in_order(&played, &[0xE0, 0xE1, 0xE4]) {
            failed.push(format!("{key} form {cmd:#x}: played {played:x?}"));
        }
    }
    assert!(failed.is_empty(), "{failed:#?}");
}

#[test]
fn revenge_nothing_without_an_ask_and_an_empty_ask_cancels() {
    let mut b = Bench::new("WEP_KEYBLADE_SO_00");
    let before = b.sora.combat().history.len();
    b.idle(300);
    assert_eq!(b.sora.combat().history.len(), before, "no attack of its own");
    b.sora.request_counter(&[0xE0], TargetId(1), false);
    b.sora.request_counter(&[], TargetId(1), false);
    b.idle(120);
    assert_eq!(b.sora.combat().history.len(), before);
    assert_eq!(b.sora.counter_state(), None);
}
