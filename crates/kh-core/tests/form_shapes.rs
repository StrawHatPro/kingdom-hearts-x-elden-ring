//! (F-SHAPES) KH3's CONVEX attack shapes in the form traces (build/sora_tmp/weapons/F-SHAPES/STATE.md): the hits land where
//! KH3's hulls are, not where the old stand-ins (a sphere of the hull's radius, an upright capsule) reached.
//! 1. Ultimate Form's transformation (StyleChange9): ChangeSW is tres_colltest01 x (5, 5, 20) with m_RotAttachType LOCAL, a
//!    20-gon of radius 500 that does not turn: a body just outside the middle of an edge (9 deg) is missed, the same
//!    distance toward a vertex (0 deg) is hit. The upright capsule of radius 500 hit both.
//! 2. Clock Drill's Clock (FINISH_DR): FinishDR / FinishDR2 are tres_colltest01 x (10, 10, 15) on the Clock's `wep` bone
//!    (roll 90 under it: the prism stands upright, 1000 wide, 750 up and down from 300 above the root). A body on the
//!    ground just inside the rim is hit (the sphere of 1000 around the centre missed it), a body hovering above the top is
//!    missed (the sphere reached it).

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

/// A standing body (the recording host's hurt capsule: radius 40, half height 90) with its centre at `centre`.
fn body(id: u32, centre: Vec3) -> TargetInfo {
    TargetInfo { id: TargetId(id), position: centre - Vec3::Z * 90.0, center: centre, velocity: Vec3::ZERO, yaw: 180.0, radius: 40.0, half_height: 90.0, flags: TargetFlags(TargetFlags::ALIVE | TargetFlags::HITTABLE | TargetFlags::ENEMY), hp: 1.0e9, hp_max: 1.0e9 }
}

struct Bench {
    sora: Sora,
    host: RecordingHost,
    world: TestWorld,
}

impl Bench {
    fn new(key: &str) -> Bench {
        Bench::facing(key, 0.0)
    }

    fn facing(key: &str, yaw: f32) -> Bench {
        let origin = Vec3::new(0.0, 0.0, Sora::origin_height_on_floor(&SORA_PARAMS));
        let sora = Sora::new(SORA_PARAMS, Pose { position: origin, yaw }).with_tuning(Tuning::GAME_EXACT).with_status(holding(key));
        let mut b = Bench { sora, host: RecordingHost::new(), world: TestWorld::floor() };
        b.idle(5);
        b
    }

    fn step(&mut self, held: &[PadButton]) -> Output {
        let set = held.iter().fold(PadButtons::NONE, |all, b| all.with(*b));
        let input = Input::default().with_pad(PadState::from_held(set, PadButtons::NONE));
        let (sora, world) = (&mut self.sora, &self.world);
        self.host.step(world, |h| sora.step_hosted(DT, &input, h))
    }

    fn idle(&mut self, n: usize) {
        for _ in 0..n {
            self.step(&[]);
        }
    }

    /// The offer of `cmd`, Triangle; returns once STYLECHANGE has begun.
    fn begin_transform(&mut self, cmd: i32) {
        self.sora.debug_add_style(100.0);
        self.idle(15);
        let list: Vec<i32> = self.sora.combat().situation.list.iter().map(|e| e.kind).collect();
        assert_eq!(list, [cmd], "the formchange is offered");
        self.step(&[PadButton::SITUATION]);
        assert_eq!(self.step(&[]).state, StateId::Stylechange);
    }

    fn until_not(&mut self, state: &[StateId]) {
        for _ in 0..3000 {
            if !state.contains(&self.step(&[]).state) {
                return;
            }
        }
        panic!("{state:?} did not end");
    }

    fn hits_on(&self, id: u32, row: &str) -> usize {
        self.host.hits().iter().filter(|(h, _)| h.target == TargetId(id) && h.attack_name.eq_ignore_ascii_case(row)).count()
    }
}

#[test]
fn ultimate_forms_change_sw_is_a_20_gon_that_does_not_turn() {
    // He faces 40 deg: m_RotAttachType LOCAL keeps the 20-gon's vertices at the world's k x 18 deg anyway.
    let mut b = Bench::facing("WEP_KEYBLADE_SO_14", 40.0);
    let at = b.sora.output().position;
    b.begin_transform(0x87);
    let yaw = b.sora.output().yaw.to_radians();
    // The hull's axis: kh_sk + (0, -900, 0) is (-0.1, 0, 900) in his actor space.
    let feet = Vec3::new(at.x - 0.1 * yaw.cos(), at.y - 0.1 * yaw.sin(), 90.0);
    let dir = |deg: f32| Vec3::new(deg.to_radians().cos(), deg.to_radians().sin(), 0.0);
    // 537 from the hull's axis: toward a vertex (radius 500) the body's 40 reaches it (37 off); toward the middle of an edge
    // (500 cos 9 deg = 493.8) it does not (43 off).
    b.host.targets.push(body(1, feet + dir(0.0) * 537.0));
    b.host.targets.push(body(2, feet + dir(9.0) * 537.0));
    b.host.targets.push(body(3, feet + dir(-90.0) * 300.0));
    b.until_not(&[StateId::Stylechange]);
    assert_eq!(b.sora.combat().form.cmd, 0x87);
    assert!(b.hits_on(3, "SCSW_CHANGE") > 0, "a body well inside is hit");
    assert!(b.hits_on(1, "SCSW_CHANGE") > 0, "toward a vertex: 37 cm off the hull, inside the body's 40");
    assert_eq!(b.hits_on(2, "SCSW_CHANGE"), 0, "toward the middle of an edge: 43 cm off (the old capsule of radius 500 hit it)");
}

/// 3. Counter Shield's finisher (B417): SH_Finish / SH_Finish2 are tres_colltest01 x (6, 6, 8) and x (0.5 .. 6, 6) at the
///    shield's ATK_01 (LOCAL: upright). They reach a body 2 m ahead, which the spheres of their m_Size 75 never did.
#[test]
fn counter_shields_finisher_hits_with_its_hulls() {
    let mut b = Bench::new("WEP_KEYBLADE_SO_01");
    let mut t = body(1, Vec3::new(200.0, 0.0, 90.0));
    t.flags = TargetFlags(t.flags.0 | TargetFlags::LOCKABLE);
    b.host.targets.push(t);
    b.begin_transform(0x89);
    b.until_not(&[StateId::Stylechange]);
    b.idle(30);
    let first = b.sora.combat().history.len();
    for k in 0..900 {
        let out = if k % 2 == 0 { b.step(&[PadButton::ATTACK]) } else { b.step(&[]) };
        if k > 30 && matches!(out.state, StateId::Idle | StateId::Run | StateId::Jump) {
            break;
        }
    }
    assert_eq!(&b.sora.combat().history[first..first + 5], [0x50, 0x51, 0x52, 0x53, 0x54], "the combo, B417 last");
    assert!(b.hits_on(1, "SCSH_FINISH") > 0 && b.hits_on(1, "SCSH_FINISH2") > 0, "{:?}", b.host.hits().iter().map(|(h, _)| h.attack_name).collect::<Vec<_>>());
}

#[test]
fn clock_drills_clock_is_an_upright_prism_on_its_bone() {
    let mut b = Bench::new("WEP_KEYBLADE_SO_11");
    b.begin_transform(0x92);
    b.until_not(&[StateId::Stylechange]);
    b.idle(60);
    b.begin_transform(0x93);
    b.until_not(&[StateId::Stylechange]);
    b.idle(30);
    assert_eq!(b.sora.combat().form.cmd, 0x93);
    // FINISH_DR: the Clock spawns at his root with his yaw.
    b.step(&[PadButton::SITUATION]);
    b.step(&[]);
    let (root, yaw) = b.sora.finish().and_then(|f| f.accompany).expect("the Clock");
    let fwd = Vec3::new(yaw.to_radians().cos(), yaw.to_radians().sin(), 0.0);
    // 1. On the ground 1030 out along his facing (a vertex of the rim, 1000): hit. The sphere of 1000 around the hull's
    //    centre (300 up) was 1044 - 40 away.
    b.host.targets.push(body(1, root + fwd * 1030.0 + Vec3::Z * 90.0));
    // 2. Hovering above the top (1050): its lowest point 60 above. The sphere reached 986 - 40 there.
    b.host.targets.push(body(2, root + fwd * 500.0 + Vec3::Z * 1200.0));
    // 3. Inside.
    b.host.targets.push(body(3, root + fwd * 400.0 + Vec3::Z * 90.0));
    b.until_not(&[StateId::Stylefinish, StateId::StylefinishDr]);
    let hits = |id: u32| ["DR2_FINISH", "DR2_FINISH_NEEDLE", "DR2_FINISH_END"].iter().map(|r| b.hits_on(id, r)).sum::<usize>();
    assert!(hits(3) > 0, "inside the Clock");
    assert!(hits(1) > 0, "the rim reaches 1000 at the floor");
    assert_eq!(hits(2), 0, "nothing above the prism's top");
    assert!(b.hits_on(3, "DR2_FINISH_NEEDLE") >= 2, "the needles hit again every AttackInterval (0.2667 s)");
}
