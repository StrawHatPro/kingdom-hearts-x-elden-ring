//! (F-ARROWGUN) The gun forms' ShooterMode (state 40) and the controller's aim (combat/forms/arrowgun/aim.rs): hold RT past
//! the tap time -> ShooterModeStart -> Double Arrowguns' Auto-Shot (DW_GATLING from the M_2416 loop, 10 a second) hitting
//! the dummy -> release -> ShooterModeEnd -> the multi-lock again; Magic Launcher's Charged Shot (BZ_CRACKER at the full
//! charge, BZ_BOMBSHOT2 on an early release); the switch list in the multi-lock and while locked on.

use kh_core::combat::forms::arrowgun;
use kh_core::magic::View;
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
    log: Vec<(StateId, u8, &'static str)>,
    held: PadButtons,
}

impl Bench {
    fn new(key: &str, dummies: &[f32]) -> Bench {
        let origin = Vec3::new(0.0, 0.0, Sora::origin_height_on_floor(&SORA_PARAMS));
        let sora = Sora::new(SORA_PARAMS, Pose { position: origin, yaw: 0.0 }).with_tuning(Tuning::GAME_EXACT).with_status(holding(key));
        let mut host = RecordingHost::new();
        for (i, x) in dummies.iter().enumerate() {
            host.targets.push(dummy(i as u32 + 1, *x));
        }
        let mut b = Bench { sora, host, world: TestWorld::floor(), log: Vec::new(), held: PadButtons::NONE };
        b.idle(5);
        b
    }

    fn step(&mut self, held: &[PadButton]) -> Output {
        let set = held.iter().fold(PadButtons::NONE, |all, b| all.with(*b));
        // A camera 400 cm behind him looking along +x (his facing), the field of view 90, 1280 x 720.
        let view = View::from_camera(Vec3::new(-400.0, 0.0, 200.0), 0.0, 0.0, 90.0, 1280, 720);
        let input = Input { view: Some(view), ..Input::default() }.with_pad(PadState::from_held(set, self.held));
        self.held = set;
        let (sora, world) = (&mut self.sora, &self.world);
        let out = self.host.step(world, |h| sora.step_hosted(DT, &input, h));
        self.log.push((out.state, out.sub_step, out.anim.name.name()));
        out
    }

    fn idle(&mut self, n: usize) {
        for _ in 0..n {
            self.step(&[]);
        }
    }

    fn clips(&self, from: usize) -> Vec<&'static str> {
        let mut out: Vec<&'static str> = Vec::new();
        for (_, _, clip) in &self.log[from..] {
            if out.last() != Some(clip) {
                out.push(clip);
            }
        }
        out
    }

    fn transform(&mut self, cmd: i32) {
        self.sora.debug_add_style(100.0);
        self.idle(15);
        let list: Vec<i32> = self.sora.combat().situation.list.iter().map(|e| e.kind).collect();
        assert!(list.contains(&cmd), "{cmd:#x} is offered: {list:x?}");
        for _ in 0..40 {
            self.step(&[PadButton::SITUATION]);
            if self.step(&[]).state == StateId::Stylechange {
                break;
            }
        }
        for _ in 0..600 {
            if self.step(&[]).state != StateId::Stylechange {
                return;
            }
        }
        panic!("STYLECHANGE did not end");
    }

    /// Hold RT until ShooterMode runs; the steps it took.
    fn hold_rt_until_shooter(&mut self) -> usize {
        for k in 0..60 {
            if self.step(&[PadButton::RightTrigger]).state == StateId::Shootermode {
                return k + 1;
            }
        }
        panic!("no ShooterMode: {:?}", &self.log[self.log.len() - 5..]);
    }

    /// Put the dummies at these distances ahead of him (along +x from where he stands now).
    fn place(&mut self, xs: &[f32]) {
        let p = self.step(&[]).position;
        for (t, x) in self.host.targets.iter_mut().zip(xs) {
            t.position = Vec3::new(p.x + x, p.y, 0.0);
            t.center = t.position + Vec3::Z * 90.0;
        }
        self.idle(2);
    }

    fn classes(&self) -> Vec<&'static str> {
        self.sora.combat().arrowgun.views().map(|b| b.class).collect()
    }
}

#[test]
fn double_arrowguns_auto_shot_fires_the_gatling_while_rt_is_held() {
    let mut b = Bench::new("WEP_KEYBLADE_SO_02", &[700.0]);
    b.transform(arrowgun::CHANGE_DW);
    assert!(b.sora.combat().arrowgun.aim.multilock, "StartStyle 0x80: the multi-lock (slot 790)");
    b.idle(30);
    let from = b.log.len();
    // RT held: a tap up to 16 frames, the hold on the 17th; SetState(40) begins in the same frame's MainProc.
    let steps = b.hold_rt_until_shooter();
    assert_eq!(steps, 17, "the hold flag past 0.2667 s");
    assert!(b.sora.combat().arrowgun.aim.shooter && !b.sora.combat().arrowgun.aim.multilock, "the shooter aim replaces the multi-lock");
    // ShooterModeStart (2404, 0.367 s), then the M_2416 loop with the camera on.
    for _ in 0..30 {
        b.step(&[PadButton::RightTrigger]);
    }
    assert!(b.clips(from).contains(&"2404") && b.clips(from).contains(&"2416"), "{:?}", b.clips(from));
    assert!(b.sora.combat().arrowgun.aim.view.camera, "the shooter camera is on");
    assert_eq!(b.sora.combat().arrowgun.aim.pick, TargetId(1), "the screen-centre pick");
    // One second of the loop: 4 TresShot notifies a 0.4 s loop = 10 bullets a second.
    let (fired, hits) = (b.sora.combat().arrowgun.fired, b.host.hits().len());
    for _ in 0..60 {
        b.step(&[PadButton::RightTrigger]);
    }
    let n = b.sora.combat().arrowgun.fired - fired;
    assert!((9..=11).contains(&n), "10 a second: {n}");
    assert!(b.classes().iter().all(|c| *c == "TresProjSS_Gatling"), "{:?}", b.classes());
    b.idle(1);
    // Release: the end request, ShooterModeEnd (2406), the state leaves; in 0x80 the multi-lock again.
    let from = b.log.len();
    b.idle(60);
    assert!(b.clips(from).contains(&"2406"), "{:?}", b.clips(from));
    assert_ne!(b.log.last().map(|l| l.0), Some(StateId::Shootermode));
    let aim = &b.sora.combat().arrowgun.aim;
    assert!(!aim.shooter && aim.multilock && !aim.view.camera, "slot 840, then slot 790");
    assert!(b.host.hits().len() > hits + 5, "the gatling hits the dummy: {}", b.host.hits().len() - hits);
    assert!(b.host.hits()[hits..].iter().any(|(r, _)| r.attack_name == "SCDW_GATLING"), "{:?}", b.host.hits()[hits..].iter().map(|(r, _)| r.attack_name).collect::<Vec<_>>());
}

#[test]
fn magic_launcher_charges_to_the_cracker_or_fires_the_bomb_shot_early() {
    let mut b = Bench::new("WEP_KEYBLADE_SO_02", &[700.0]);
    b.transform(arrowgun::CHANGE_DW);
    b.idle(30);
    b.transform(arrowgun::CHANGE_BZ);
    assert_eq!(b.sora.combat().form.cmd, arrowgun::CHANGE_BZ);
    assert!(!b.sora.combat().arrowgun.aim.multilock, "StartStyle's common part ends the multi-lock");
    b.idle(30);
    b.place(&[700.0]);
    // Hold: Start (0.367 s), then the charge 120 a second: full after 0.833 s, BZ_CRACKER at once.
    b.hold_rt_until_shooter();
    let fired = b.sora.combat().arrowgun.fired;
    let mut charged_at = None;
    let from = b.log.len();
    for k in 0..120 {
        b.step(&[PadButton::RightTrigger]);
        if b.sora.combat().arrowgun.aim.view.charged {
            charged_at = Some(k);
            break;
        }
    }
    let k = charged_at.expect("the charge is full");
    // 0.367 s of the start clip (22 frames) + 100 / 120 s (50 frames).
    assert!((68..=76).contains(&k), "full after the start and 0.833 s: frame {k}");
    for _ in 0..4 {
        b.step(&[PadButton::RightTrigger]);
    }
    assert_eq!(b.sora.combat().arrowgun.fired - fired, 1, "one Charged Shot");
    assert!(b.clips(from).contains(&"5416"), "M_5416: {:?}", b.clips(from));
    assert!(b.classes().iter().any(|c| c.starts_with("TresProjSS_Cracker")), "{:?}", b.classes());
    b.idle(120);
    b.place(&[700.0]);
    // An early release: BZ_BOMBSHOT2 with M_5415.
    let fired = b.sora.combat().arrowgun.fired;
    b.hold_rt_until_shooter();
    for _ in 0..30 {
        b.step(&[PadButton::RightTrigger]);
    }
    assert!(!b.sora.combat().arrowgun.aim.view.charged);
    let from = b.log.len();
    b.idle(3);
    assert_eq!(b.sora.combat().arrowgun.fired - fired, 1, "the release fires");
    assert!(b.classes().iter().any(|c| c.starts_with("TresProjSS_BombShot2")), "{:?}", b.classes());
    b.idle(80);
    assert!(b.clips(from).contains(&"5415") && b.clips(from).contains(&"5406"), "{:?}", b.clips(from));
    assert_ne!(b.log.last().map(|l| l.0), Some(StateId::Shootermode));
}

#[test]
fn the_switch_list_in_the_multilock_and_while_locked() {
    let mut b = Bench::new("WEP_KEYBLADE_SO_02", &[600.0, 300.0]);
    b.idle(3);
    assert!(b.sora.combat().arrowgun.aim.switch_list.is_empty(), "base form, not locked: no list");
    b.transform(arrowgun::CHANGE_DW);
    b.place(&[600.0, 300.0]);
    // The multi-lock: both, nearest first.
    assert_eq!(b.sora.combat().arrowgun.aim.switch_list, vec![TargetId(2), TargetId(1)]);
    // RB tap: slot 791(1), the lock on the nearest.
    b.step(&[PadButton::RightShoulder]);
    b.idle(2);
    let lock = b.sora.combat().lockon;
    assert!(lock.locked && lock.current == TargetId(2) && !b.sora.combat().arrowgun.aim.multilock, "{lock:?}");
    assert_eq!(b.sora.combat().arrowgun.aim.switch_list.len(), 2, "while locked the list is rebuilt every frame");
    // RB again unlocks: the multi-lock comes back in 0x80.
    b.step(&[PadButton::RightShoulder]);
    b.idle(2);
    assert!(!b.sora.combat().lockon.locked && b.sora.combat().arrowgun.aim.multilock);
}
