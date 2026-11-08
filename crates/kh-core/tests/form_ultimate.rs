//! (weapons F-ULTIMATE) Ultimate Form (Ultima Weapon, CHANGE_SW 0x87): the trace of the form from the offer to its end
//! (build/sora_tmp/weapons/F-ULTIMATE/STATE.md): offer -> STYLECHANGE (StyleChange9, wear form 6 Final, w_so192 in his hand,
//! the transformation's ChangeSW hit) -> the COMBO_SW combo (0xC7 .. 0xCC: the blade flurry, the stabs, the raid, the dive,
//! the two finishers) -> the teleport toward a far target -> the Reflec guard and its counter (0x25, BladeCounter) ->
//! FINISH_SW (M_9419, the blades' storm) -> the form's end.

use kh_core::enums::ETresDamageKind;
use kh_core::host::EventKind;
use kh_core::states::HitOnPlayer;
use kh_core::status::LoadoutValue;
use kh_core::test_world::TestWorld;
use kh_core::{Input, Loadout, Output, PadButton, PadButtons, PadState, Pose, RecordingHost, Sora, StateId, Status, TargetFlags, TargetId, TargetInfo, Tuning, Vec3, SORA_PARAMS};

const DT: f32 = 1.0 / 60.0;
const ULTIMA: &str = "WEP_KEYBLADE_SO_14";

fn cell(json: &serde_json::Value) -> Option<LoadoutValue> {
    match json {
        serde_json::Value::Number(n) => n.as_f64().map(LoadoutValue::Number),
        serde_json::Value::String(s) => Some(LoadoutValue::Text(s.clone())),
        serde_json::Value::Array(items) => items.iter().map(|i| i.as_str().map(str::to_string)).collect::<Option<Vec<_>>>().map(LoadoutValue::List),
        serde_json::Value::Object(map) => map.iter().map(|(k, v)| v.as_f64().map(|n| (k.clone(), n))).collect::<Option<Vec<_>>>().map(LoadoutValue::Map),
        _ => None,
    }
}

/// The loadout with only the abilities named equipped.
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

fn dummy(id: u32, x: f32) -> TargetInfo {
    TargetInfo { id: TargetId(id), position: Vec3::new(x, 0.0, 0.0), center: Vec3::new(x, 0.0, 90.0), velocity: Vec3::ZERO, yaw: 180.0, radius: 40.0, half_height: 90.0, flags: TargetFlags(TargetFlags::ALIVE | TargetFlags::HITTABLE | TargetFlags::LOCKABLE | TargetFlags::ENEMY), hp: 1.0e9, hp_max: 1.0e9 }
}

struct Bench {
    sora: Sora,
    host: RecordingHost,
    world: TestWorld,
    /// Every step: (state, sub-step, clip, the events).
    log: Vec<(StateId, u8, &'static str, Vec<EventKind>)>,
}

impl Bench {
    fn new(at: f32) -> Bench {
        Bench::with(at, &[])
    }

    fn with(at: f32, abilities: &[&str]) -> Bench {
        let origin = Vec3::new(0.0, 0.0, Sora::origin_height_on_floor(&SORA_PARAMS));
        let sora = Sora::new(SORA_PARAMS, Pose { position: origin, yaw: 0.0 }).with_tuning(Tuning::GAME_EXACT).with_status(holding(ULTIMA, abilities));
        let mut host = RecordingHost::new();
        host.targets.push(dummy(1, at));
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

    fn transform(&mut self) -> usize {
        self.sora.debug_add_style(100.0);
        self.idle(15);
        let list: Vec<i32> = self.sora.combat().situation.list.iter().map(|e| e.kind).collect();
        assert_eq!(list, [0x87], "the formchange is offered");
        let from = self.log.len();
        self.step(&[PadButton::SITUATION]);
        assert_eq!(self.step(&[]).state, StateId::Stylechange);
        for _ in 0..600 {
            if self.step(&[]).state != StateId::Stylechange {
                return from;
            }
        }
        panic!("STYLECHANGE did not end");
    }

    /// Attack presses every other step until the chain ends; the kinds played.
    fn combo(&mut self) -> Vec<u8> {
        let first = self.sora.combat().history.len();
        for k in 0..1800 {
            let out = if k % 2 == 0 { self.step(&[PadButton::ATTACK]) } else { self.step(&[]) };
            if k > 30 && matches!(out.state, StateId::Idle | StateId::Run | StateId::Jump) {
                break;
            }
        }
        self.sora.combat().history[first..].to_vec()
    }

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

    fn hits_with(&self, row: &str) -> usize {
        self.host.hits().iter().filter(|(h, _)| h.attack_name.eq_ignore_ascii_case(row)).count()
    }
}

#[test]
fn ultimate_form_from_the_offer_to_its_end() {
    let mut b = Bench::new(150.0);
    assert_eq!(kh_core::combat::forms::moveset_name("form.SW"), "ultimate");
    // ---- offer -> transform: StyleChange9, wear form 6 (Final), the Ultima Weapon in his hand, ChangeSW hits
    let from = b.transform();
    let f = b.sora.combat().form;
    assert_eq!((f.cmd, f.level, f.time_full, f.wear_form), (0x87, 1, 30.0, 6), "Ultimate Form: 30 s, wear form 6 (Final)");
    assert!(b.clips(from).contains(&"StyleChange9"), "{:?}", b.clips(from));
    assert!(f.weapon_units()[0].contains("w_so192"), "{:?}", f.weapon_units());
    assert!(b.hits_with("SCSW_CHANGE") > 0, "the transformation's ChangeSW (w_so191) hit the dummy");
    assert_eq!(b.sora.combat().situation.slot_b.map(|e| e.kind), Some(0xA1), "FINISH_SW on offer");
    assert_eq!(b.sora.combat().lockon.soft_range, 1800.0, "StartStyle 0x87: the soft-target range x 2");
    // ---- the combo: COMBO_SW 0xC7 .. 0xCC near one target
    b.idle(60);
    let from = b.log.len();
    let kinds = b.combo();
    assert_eq!(kinds, [0xC7, 0xC8, 0xC9, 0xCA, 0xCB, 0xCC], "the six steps of COMBO_SW");
    assert!(b.log[from..].iter().any(|l| l.0 == StateId::ComboSw), "the attack class is COMBO_SW");
    let clips = b.clips(from);
    for c in ["9411", "9412", "9413", "9416", "9417", "9418"] {
        assert!(clips.contains(&c), "{c} not played: {clips:?}");
    }
    for row in ["SCSW_RENGEKI", "SCSW_RENGEKI3", "SCSW_STAB", "SCSW_RAID", "SCSW_DIVE", "SCSW_FINISH", "SCSW_FINISH2"] {
        assert!(b.hits_with(row) > 0, "{row} hit the dummy");
    }
    assert_eq!(b.sora.combat().form.cmd, 0x87, "still in the form");
}

/// A target farther than 400 cm (within the doubled soft range): the step vanishes (JumpNormalStart, invincible) and
/// reappears 150 cm short of it after 0.2 s.
#[test]
fn the_teleport_toward_a_far_target() {
    let mut b = Bench::new(1200.0);
    b.transform();
    b.idle(30);
    assert_eq!(b.sora.combat().lockon.current, TargetId(1), "the soft target at 12 m (range 18 m in the form)");
    let from = b.log.len();
    b.step(&[PadButton::ATTACK]);
    b.step(&[]);
    assert_eq!(b.log.last().map(|l| l.2), Some("JumpNormalStart"), "the vanish: {:?}", b.clips(from));
    assert!(b.sora.status().damage_invalid, "invincible while away");
    b.idle(14);
    let x = b.sora.output().position.x;
    assert!((1000.0..1100.0).contains(&x), "150 cm short of the target's lock point: {x}");
    assert!(b.clips(from).contains(&"9411"), "then the kind plays: {:?}", b.clips(from));
    b.idle(60);
    assert!(!b.sora.status().damage_invalid, "the invincibility ends 0.5 s after (or at TresEnableChange)");
}

/// The Reflec guard (Guard slot 119): AnimSet9's Reflec without REFLECT_GUARD; a hit on it opens the counter window and the
/// attack is the forced kind 0x25 (BarrierCracker 9341) whose shot BARRIERCRACKER (TresProjSW_BladeCounter) hits.
#[test]
fn the_reflec_guard_and_its_counter_blade() {
    // GUARD_COUNTER opens the counter window (FUN_1414145C0); REFLECT_GUARD is not needed in weapon form 9 (CanGuard).
    let mut b = Bench::with(150.0, &["GUARD_COUNTER"]);
    b.transform();
    b.idle(60);
    let from = b.log.len();
    b.step(&[PadButton::GUARD]);
    b.idle(3);
    assert_eq!(b.log.last().map(|l| l.0), Some(StateId::Guard));
    assert!(b.clips(from).contains(&"9279"), "Reflec of AnimSet9: {:?}", b.clips(from));
    let own = b.sora.output().position;
    let hp = b.sora.status().hp;
    b.sora.take_hit(HitOnPlayer { direction: Vec3::new(-1.0, 0.0, 0.0), causer_yaw: 180.0, ..HitOnPlayer::new(ETresDamageKind::Small, 30.0, TargetId(1), own + Vec3::new(150.0, 0.0, 0.0)) });
    b.idle(2);
    assert_eq!(b.sora.status().hp, hp, "guarded");
    let from = b.log.len();
    b.step(&[PadButton::ATTACK]);
    b.idle(70);
    assert_eq!(b.sora.combat().history.last(), Some(&0x25), "the guard counter of the form");
    assert!(b.clips(from).contains(&"9341"), "BarrierCracker of AnimSet9: {:?}", b.clips(from));
    assert!(b.hits_with("SCSW_COUNTER") > 0, "BladeCounter hit the dummy");
}

/// FINISH_SW: M_9419 (SW_Myriad, SW_Myriad2) and the form's end.
#[test]
fn finish_sw_ends_the_form() {
    let mut b = Bench::new(150.0);
    b.transform();
    b.idle(30);
    let from = b.log.len();
    let subs = b.finish();
    assert_eq!(subs, [4, 5], "StyleFinish slot 151's sub-steps (0 runs in the first tick)");
    assert!(b.clips(from).contains(&"M_9419"), "{:?}", b.clips(from));
    assert!(b.hits_with("SCSW_MYRIAD") > 0 && b.hits_with("SCSW_MYRIAD_END") > 0, "the storm's hits");
    b.idle(5);
    let f = b.sora.combat().form;
    assert_eq!((f.cmd, f.wear_form), (0, 0), "the finish ends the form");
    assert_eq!(b.sora.combat().lockon.soft_range, 900.0, "EndStyle: the range back");
}
