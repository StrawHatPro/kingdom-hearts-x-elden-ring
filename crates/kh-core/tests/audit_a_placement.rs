//! (AUDIT-A PLACEMENT) The thrown form parts through the REAL core (kh-core combat::forms::placement): Counter Shield's raid
//! (M_B413's TresAnimNotify_ThrowWeapon at 0.3005 s throws w_so032, its M_B414 looped; the recall before M_B415) and Storm
//! Flag's flag throw (M_F412 at 0.2766 s throws w_so093, the recall before M_F414): where KH3 puts the weapon (FreeMove: its
//! socket at the throw, then 2000 cm/s along his forward, homing on the attack's target at 360 deg/s after 0.1 s), and that
//! it is back on his hand when the recall clip starts. The trace goes to build/sora_tmp/weapons/AUDIT-A/logs/.

use kh_core::status::LoadoutValue;
use kh_core::test_world::TestWorld;
use kh_core::{Input, Loadout, Output, PadButton, PadButtons, PadState, Pose, RecordingHost, Sora, StateId, Status, TargetFlags, TargetId, TargetInfo, Tuning, Vec3, SORA_PARAMS};
use std::fmt::Write as _;

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
        let value = if k == "abilities.equipped" { LoadoutValue::List(vec!["REFLECT_GUARD".to_string()]) } else { cell(&row[1]).unwrap() };
        loadout.set(k, &value).unwrap_or_else(|e| panic!("{e}"));
    }
    loadout.resolve().unwrap();
    let mut s = Status::from_loadout(&loadout);
    s.weapon = key.to_string();
    s
}

fn dummy(x: f32) -> TargetInfo {
    TargetInfo { id: TargetId(1), position: Vec3::new(x, 0.0, 0.0), center: Vec3::new(x, 0.0, 90.0), velocity: Vec3::ZERO, yaw: 180.0, radius: 40.0, half_height: 90.0, flags: TargetFlags(TargetFlags::ALIVE | TargetFlags::HITTABLE | TargetFlags::LOCKABLE | TargetFlags::ENEMY), hp: 1.0e6, hp_max: 1.0e6 }
}

/// One step of the log: (state, clip, clip time, Sora's position, yaw, the thrown parts (model, position)).
type Step = (StateId, &'static str, f32, Vec3, f32, Vec<(&'static str, Vec3)>);

/// The throw to check: (model, the throw clip, the weapon clip's row of Sora's, the recall clip, the notify time, the socket at
/// the throw in his actor frame).
type Throw<'a> = (&'a str, &'a str, &'a str, &'a str, f32, Vec3);

struct Bench {
    sora: Sora,
    host: RecordingHost,
    world: TestWorld,
    /// Every step: (state, clip, clip time, Sora's position, yaw, the thrown parts (model, position)).
    log: Vec<Step>,
    trace: String,
}

impl Bench {
    fn new(key: &str, x: f32) -> Bench {
        let origin = Vec3::new(0.0, 0.0, Sora::origin_height_on_floor(&SORA_PARAMS));
        let sora = Sora::new(SORA_PARAMS, Pose { position: origin, yaw: 0.0 }).with_tuning(Tuning::GAME_EXACT).with_status(holding(key));
        let mut host = RecordingHost::new();
        host.targets.push(dummy(x));
        let mut b = Bench { sora, host, world: TestWorld::floor(), log: Vec::new(), trace: String::new() };
        b.idle(5);
        b
    }

    fn step(&mut self, held: &[PadButton]) -> Output {
        let set = held.iter().fold(PadButtons::NONE, |all, b| all.with(*b));
        let input = Input::default().with_pad(PadState::from_held(set, PadButtons::NONE));
        let (sora, world) = (&mut self.sora, &self.world);
        let out = self.host.step(world, |h| sora.step_hosted(DT, &input, h));
        let thrown: Vec<(&'static str, Vec3)> = self.sora.combat().thrown.iter().map(|t| (t.model, t.position)).collect();
        let _ = write!(self.trace, "{} {:?} {}@{:.3} sora ({:.0}, {:.0}, {:.0}) yaw {:.0}", self.log.len(), out.state, out.anim.name.name(), out.anim.time, out.position.x, out.position.y, out.position.z, out.yaw);
        for t in &self.sora.combat().thrown {
            let (logical, _) = t.clip();
            let _ = write!(self.trace, " | THROWN {} at ({:.1}, {:.1}, {:.1}) v ({:.0}, {:.0}, {:.0}) {logical}@{:.3}", t.model, t.position.x, t.position.y, t.position.z, t.velocity.x, t.velocity.y, t.velocity.z, t.clip_time());
        }
        self.trace.push('\n');
        self.log.push((out.state, out.anim.name.name(), out.anim.time, out.position, out.yaw, thrown));
        out
    }

    fn idle(&mut self, n: usize) {
        for _ in 0..n {
            self.step(&[]);
        }
    }

    fn transform(&mut self) {
        self.sora.debug_add_style(100.0);
        self.idle(15);
        self.step(&[PadButton::SITUATION]);
        assert_eq!(self.step(&[]).state, StateId::Stylechange);
        for _ in 0..600 {
            if self.step(&[]).state != StateId::Stylechange {
                return;
            }
        }
        panic!("STYLECHANGE did not end");
    }

    fn save(&self, name: &str) {
        let dir = format!("{}/../../build/sora_tmp/weapons/AUDIT-A/logs", env!("CARGO_MANIFEST_DIR"));
        if std::path::Path::new(&dir).is_dir() {
            let _ = std::fs::write(format!("{dir}/{name}.trace"), &self.trace);
        }
    }

    /// The thrown part's first and last step and the clip names Sora played while it flew; asserts KH3's rules.
    fn check_throw(&self, from: usize, (model, throw_clip, flight_clip, recall_clip, at_time, offset): Throw<'_>) {
        let log = &self.log[from..];
        let on = |l: &Step| l.5.iter().any(|t| t.0 == model);
        let first = log.iter().position(on).unwrap_or_else(|| panic!("{model} never thrown"));
        let last = log.iter().rposition(on).unwrap();
        // thrown in the throw clip at its notify time (the pose tick that passed it)
        // (his place before that step's move: the notify fires in the pose tick, before the movement applies its root motion)
        let (_, clip, time, _, _, ref thrown) = log[first];
        let (sora_at, yaw) = (log[first - 1].3, log[first - 1].4);
        assert_eq!(clip, throw_clip, "thrown during {clip}");
        assert!(time >= at_time && time < at_time + DT * 1.5, "thrown at {time} (the notify at {at_time})");
        // at his socket at the throw (his actor frame, from the generated row)
        let (sin, cos) = yaw.to_radians().sin_cos();
        let socket = sora_at + Vec3::new(offset.x * cos - offset.y * sin, offset.x * sin + offset.y * cos, offset.z);
        let p0 = thrown.iter().find(|t| t.0 == model).unwrap().1;
        assert!(p0.distance(socket) < 1.0, "{model} starts at his socket: {p0} vs {socket}");
        // it flies (2000 cm/s): 1 s later it is metres from where it left
        let later = log.get(first + 30).and_then(|l| l.5.iter().find(|t| t.0 == model)).map(|t| t.1);
        if let Some(p) = later {
            assert!(p.distance(p0) > 300.0, "{model} flies: {p0} -> {p}");
        }
        // it flies through the throw clip and the flight clip, and is back (recalled) when the recall clip starts
        assert!(log[first..=last].iter().all(|l| l.1 == throw_clip || l.1 == flight_clip), "{model} flies only during {throw_clip} / {flight_clip}");
        let after = &log[last + 1];
        assert_eq!(after.1, recall_clip, "recalled when {recall_clip} starts (it was {})", after.1);
        assert!(after.5.is_empty(), "back on his hand");
    }
}

fn socket_of(id: &str) -> Vec3 {
    let r = kh_core::combat::forms::placement::FORM_PLACEMENT.iter().find(|r| r.id == id).unwrap_or_else(|| panic!("{id}"));
    Vec3::new(r.values[5], r.values[6], r.values[7])
}

#[test]
fn counter_shields_raid_throws_the_shield_and_recalls_it() {
    let mut b = Bench::new("WEP_KEYBLADE_SO_01", 200.0);
    b.transform();
    assert_eq!(b.sora.combat().form.cmd, 0x89);
    b.idle(30);
    // 0x50 0x51 then the raid 0x52 (M_B413 -> M_B414 -> M_B415): two presses, then the third alone
    let from = b.log.len();
    for _ in 0..600 {
        if b.sora.combat().history.last() == Some(&0x52) {
            break;
        }
        b.step(&[PadButton::ATTACK]);
        b.step(&[]);
    }
    b.idle(150);
    b.save("placement_shield_raid");
    b.check_throw(from, ("w_so032", "B413", "B414", "B415", 0.3005, socket_of("throw.SH.1.B413.w_so032")));
}

#[test]
fn storm_flags_throw_throws_the_flag_and_recalls_it() {
    let mut b = Bench::new("WEP_KEYBLADE_SO_09", 800.0);
    b.transform();
    b.idle(30);
    b.transform();
    assert_eq!(b.sora.combat().form.cmd, 0x8D);
    b.idle(30);
    // the target 800 cm ahead of him (> 600: the flag throw 0xA4 first)
    let (p, yaw) = (b.log.last().unwrap().3, b.log.last().unwrap().4);
    let (sin, cos) = yaw.to_radians().sin_cos();
    let x = Vec3::new(p.x + cos * 800.0, p.y + sin * 800.0, 0.0);
    (b.host.targets[0].position, b.host.targets[0].center) = (x, x + Vec3::Z * 90.0);
    b.idle(5);
    let from = b.log.len();
    b.step(&[PadButton::ATTACK]);
    b.idle(150);
    b.save("placement_flag_throw");
    assert_eq!(b.sora.combat().history.last(), Some(&0xA4), "far: the flag throw");
    b.check_throw(from, ("w_so093", "F412", "F413", "F414", 0.2766, socket_of("throw.FL.1.F412.w_so093")));
}
