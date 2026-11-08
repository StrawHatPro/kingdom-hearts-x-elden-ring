//! (AUDIT-A THROW) The THROWN form part's hits through the REAL core: KH3 activates a TresAnimNotifyState_Attack group on
//! every equipped weapon's own TresAtkColl0 (pawn interface slot 64 0x1405B3FE0 -> Activate 0x140625500), whose shapes hang
//! on the weapon mesh (Update 0x140647950, SocketTransform 0x14064BA60). A thrown weapon (FreeMove 0x14177DB30) is not
//! detached, so its shapes fly with it (kh-core combat/forms/placement.rs `thrown_shape`, hooked in combat/hit.rs `shape`).
//! Before, the raid's / flag throw's windows swept the shapes baked at Sora's hand (the `track` rows). Proof: a dummy 5 m in
//! front of Sora (out of every in-hand reach) is hit by Counter Shield's flying shield in the raid, and a far dummy by
//! Storm Flag's flying flag, each hit at the flying part. Traces go to build/sora_tmp/weapons/AUDIT-A/logs/.

use kh_core::status::LoadoutValue;
use kh_core::test_world::TestWorld;
use kh_core::host::Call;
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

/// One accepted hit: (step, Sora's clip, his position, the hit's row and point, the thrown part's shape then: ends, radius).
type HitLine = (usize, &'static str, Vec3, &'static str, Vec3, Option<(Vec3, Vec3, f32)>);

struct Bench {
    sora: Sora,
    host: RecordingHost,
    world: TestWorld,
    steps: usize,
    /// His place and yaw after the last step.
    last: (Vec3, f32),
    hits: Vec<HitLine>,
    trace: String,
}

impl Bench {
    fn new(key: &str, x: f32) -> Bench {
        let origin = Vec3::new(0.0, 0.0, Sora::origin_height_on_floor(&SORA_PARAMS));
        let sora = Sora::new(SORA_PARAMS, Pose { position: origin, yaw: 0.0 }).with_tuning(Tuning::GAME_EXACT).with_status(holding(key));
        let mut host = RecordingHost::new();
        host.targets.push(dummy(x));
        let mut b = Bench { sora, host, world: TestWorld::floor(), steps: 0, last: (origin, 0.0), hits: Vec::new(), trace: String::new() };
        b.idle(5);
        b
    }

    fn step(&mut self, held: &[PadButton]) -> Output {
        let set = held.iter().fold(PadButtons::NONE, |all, b| all.with(*b));
        let input = Input::default().with_pad(PadState::from_held(set, PadButtons::NONE));
        let calls = self.host.calls.len();
        let (sora, world) = (&mut self.sora, &self.world);
        let out = self.host.step(world, |h| sora.step_hosted(DT, &input, h));
        let combat = self.sora.combat();
        let window = combat.windows.first().map(|w| w.group);
        let shape = window.and_then(|g| kh_core::combat::forms::placement::thrown_shape(&combat.thrown, g));
        let _ = write!(self.trace, "{} {:?} {}@{:.3} sora ({:.0}, {:.0}, {:.0})", self.steps, out.state, out.anim.name.name(), out.anim.time, out.position.x, out.position.y, out.position.z);
        for t in &combat.thrown {
            let _ = write!(self.trace, " | THROWN {} at ({:.1}, {:.1}, {:.1}) {}@{:.3}", t.model, t.position.x, t.position.y, t.position.z, t.clip().0, t.clip_time());
        }
        if let (Some(g), Some((a, b, r))) = (window, shape) {
            let _ = write!(self.trace, " | window {g} ON THE THROWN PART ({:.1}, {:.1}, {:.1})-({:.1}, {:.1}, {:.1}) r {r}", a.x, a.y, a.z, b.x, b.y, b.z);
        } else if let Some(g) = window {
            let _ = write!(self.trace, " | window {g} (in hand)");
        }
        for c in &self.host.calls[calls..] {
            if let Call::Hit(h, r) = c {
                if r.accepted {
                    let _ = write!(self.trace, " | HIT {} at ({:.1}, {:.1}, {:.1}), {:.0} cm from Sora", h.attack_name, h.position.x, h.position.y, h.position.z, h.position.distance(out.position));
                    self.hits.push((self.steps, out.anim.name.name(), out.position, h.attack_name, h.position, shape));
                }
            }
        }
        self.trace.push('\n');
        self.steps += 1;
        self.last = (out.position, out.yaw);
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

    /// The dummy `d` cm ahead of Sora now (along his yaw, on the floor).
    fn dummy_ahead(&mut self, d: f32) {
        let (p, yaw) = self.last;
        let (sin, cos) = yaw.to_radians().sin_cos();
        let x = Vec3::new(p.x + cos * d, p.y + sin * d, 0.0);
        (self.host.targets[0].position, self.host.targets[0].center) = (x, x + Vec3::Z * 90.0);
    }

    fn save(&self, name: &str) {
        let dir = format!("{}/../../build/sora_tmp/weapons/AUDIT-A/logs", env!("CARGO_MANIFEST_DIR"));
        if std::path::Path::new(&dir).is_dir() {
            let _ = std::fs::write(format!("{dir}/{name}.trace"), &self.trace);
        }
    }
}

/// Distance of `p` from the segment a-b.
fn off_segment(p: Vec3, a: Vec3, b: Vec3) -> f32 {
    let ab = b - a;
    let t = if ab.length_squared() > 1.0e-6 { ((p - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0) } else { 0.0 };
    p.distance(a + ab * t)
}

/// Every hit of `row` lands ON the flying part's shape (within its radius of its segment) and far out of his reach.
fn check_hits(b: &Bench, row: &str, clips: &[&str]) {
    let hits: Vec<&HitLine> = b.hits.iter().filter(|h| h.3 == row).collect();
    assert!(!hits.is_empty(), "the dummy 5 m away is hit by {row} (trace: no HIT line)");
    for (step, clip, sora, _, point, shape) in hits {
        assert!(clips.contains(clip), "step {step}: hit during {clip}");
        let (a, s_b, radius) = shape.unwrap_or_else(|| panic!("step {step}: the window was not on the thrown part"));
        let off = off_segment(*point, a, s_b);
        assert!(off <= radius + 1.0, "step {step}: the hit point is {off} cm from the flying part's shape (radius {radius})");
        assert!(point.distance(*sora) > 300.0, "step {step}: hit {} cm from Sora: out of every in-hand reach", point.distance(*sora));
    }
}

#[test]
fn counter_shields_flying_shield_hits_a_dummy_5_m_away() {
    let mut b = Bench::new("WEP_KEYBLADE_SO_01", 200.0);
    b.transform();
    assert_eq!(b.sora.combat().form.cmd, 0x89);
    b.idle(30);
    // 0x50 0x51, then the raid 0x52 (M_B413 -> M_B414 -> M_B415): the dummy goes 5 m ahead of him when M_B413 starts
    let mut placed = false;
    for _ in 0..600 {
        let out = b.step(&[PadButton::ATTACK]);
        if !placed && out.anim.name.name() == "B413" {
            b.dummy_ahead(500.0);
            placed = true;
            break;
        }
        let out = b.step(&[]);
        if !placed && out.anim.name.name() == "B413" {
            b.dummy_ahead(500.0);
            placed = true;
            break;
        }
    }
    assert!(placed, "the raid M_B413 played");
    b.idle(150);
    b.save("throw_shield_raid");
    check_hits(&b, "SCSH_RAID", &["B413", "B414"]);
}

#[test]
fn storm_flags_flying_flag_hits_a_far_dummy() {
    let mut b = Bench::new("WEP_KEYBLADE_SO_09", 800.0);
    b.transform();
    b.idle(30);
    b.transform();
    assert_eq!(b.sora.combat().form.cmd, 0x8D);
    b.idle(30);
    b.dummy_ahead(800.0);
    b.idle(5);
    b.step(&[PadButton::ATTACK]);
    b.idle(150);
    b.save("throw_flag_throw");
    assert_eq!(b.sora.combat().history.last(), Some(&0xA4), "far: the flag throw");
    check_hits(&b, "SCFL_RAID", &["F412", "F413"]);
}
