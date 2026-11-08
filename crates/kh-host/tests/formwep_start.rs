//! (FORMWEP-FIX) THE FRAMES BEFORE A FORM CHANGE: from the step a form starts (kh-core StartStyle, the Triangle press) to the
//! first step of its transformation (StyleChange), the hosts draw the HAND KEYBLADE, never the new form's weapon (a Hyper
//! Hammer popped into his hand for a frame or two ahead of the Keyblade's transformation: `Form::equip` still says
//! `Weapons` until StyleChange's Begin). Driven through the real core for every transforming Keyblade's first stage and
//! Favorite Deputy's second, with the draw rule of kh-er sora.rs `submit` (the form's units while a form row is held,
//! `equip` is not `Keyblade` and the form-start gate is off: kh-host formstart.rs).

use kh_core::combat::form::FormEquip;
use kh_core::status::LoadoutValue;
use kh_core::test_world::TestWorld;
use kh_core::{Input, Loadout, PadButton, PadButtons, PadState, Pose, RecordingHost, Sora, StateId, Status, TargetFlags, TargetId, TargetInfo, Tuning, Vec3, SORA_PARAMS};
use kh_host::formstart::FormStartGate;

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

/// Level 99, no equipped abilities, holding `key`.
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

/// One drawn step: the state, whether a form started (kh-core alone would draw its units), whether the host draws them.
struct Drawn {
    state: StateId,
    core_rule: bool,
    host_rule: bool,
}

struct Bench {
    sora: Sora,
    host: RecordingHost,
    world: TestWorld,
    gate: FormStartGate,
}

impl Bench {
    fn new(key: &str) -> Bench {
        let origin = Vec3::new(0.0, 0.0, Sora::origin_height_on_floor(&SORA_PARAMS));
        let sora = Sora::new(SORA_PARAMS, Pose { position: origin, yaw: 0.0 }).with_tuning(Tuning::GAME_EXACT).with_status(holding(key));
        let mut host = RecordingHost::new();
        host.targets.push(TargetInfo { id: TargetId(1), position: Vec3::new(200.0, 0.0, 0.0), center: Vec3::new(200.0, 0.0, 90.0), velocity: Vec3::ZERO, yaw: 180.0, radius: 40.0, half_height: 90.0, flags: TargetFlags(TargetFlags::ALIVE | TargetFlags::HITTABLE | TargetFlags::LOCKABLE | TargetFlags::ENEMY), hp: 1.0e6, hp_max: 1.0e6 });
        let mut b = Bench { sora, host, world: TestWorld::floor(), gate: FormStartGate::default() };
        for _ in 0..5 {
            b.step(&[]);
        }
        b
    }

    /// One step and the draw decision after it (kh-er sora.rs `submit`'s rule, its gate stepped as `step_chain` does).
    fn step(&mut self, held: &[PadButton]) -> Drawn {
        let set = held.iter().fold(PadButtons::NONE, |all, b| all.with(*b));
        let input = Input::default().with_pad(PadState::from_held(set, PadButtons::NONE));
        let (sora, world) = (&mut self.sora, &self.world);
        let out = self.host.step(world, |h| sora.step_hosted(DT, &input, h));
        let form = self.sora.combat().form;
        let gate = self.gate.step(&form, out.state);
        let core_rule = form.row().is_some_and(|r| r.weapon_form_value > 1) && form.equip != FormEquip::Keyblade;
        Drawn { state: out.state, core_rule, host_rule: core_rule && !gate }
    }

    /// The gauge filled, the offer, Triangle, then every step to the first one of the transformation (and 10 more).
    fn change(&mut self) -> Vec<Drawn> {
        self.sora.debug_add_style(100.0);
        for _ in 0..15 {
            self.step(&[]);
        }
        assert!(!self.sora.combat().situation.list.is_empty(), "a formchange is offered");
        let mut steps = vec![self.step(&[PadButton::SITUATION])];
        for _ in 0..30 {
            steps.push(self.step(&[]));
            if steps.iter().filter(|d| d.state == StateId::Stylechange).count() > 10 {
                break;
            }
        }
        steps
    }
}

/// The steps before the transformation's first one: the host never draws the form's units there; returns how many of
/// them kh-core's own `equip` would have drawn them on.
fn check(name: &str, steps: &[Drawn]) -> usize {
    let first = steps.iter().position(|d| d.state == StateId::Stylechange).unwrap_or_else(|| panic!("{name}: the transformation never began"));
    for (k, d) in steps[..first].iter().enumerate() {
        assert!(!d.host_rule, "{name}: step {k} before the transformation ({:?}) draws the form's weapon", d.state);
    }
    steps[..first].iter().filter(|d| d.core_rule).count()
}

#[test]
fn the_keyblade_stays_in_his_hand_until_every_form_change_begins() {
    let mut popped = 0;
    for key in ["WEP_KEYBLADE_SO_02", "WEP_KEYBLADE_SO_01", "WEP_KEYBLADE_SO_07", "WEP_KEYBLADE_SO_03", "WEP_KEYBLADE_SO_05", "WEP_KEYBLADE_SO_04", "WEP_KEYBLADE_SO_06", "WEP_KEYBLADE_SO_09", "WEP_KEYBLADE_SO_11", "WEP_KEYBLADE_SO_10", "WEP_KEYBLADE_SO_08", "WEP_KEYBLADE_SO_14"] {
        let mut b = Bench::new(key);
        let steps = b.change();
        popped += check(key, &steps);
        // once the transformation is over the form's units are drawn again (the gate never lingers)
        for _ in 0..600 {
            if b.step(&[]).state != StateId::Stylechange {
                break;
            }
        }
        assert!(b.step(&[]).host_rule, "{key}: the form's weapon after its transformation");
    }
    assert!(popped > 0, "kh-core's own rule draws the new weapon before the transformation at least once (the pop this gate hides)");
}

#[test]
fn the_second_stage_shows_the_keyblade_until_its_change_begins() {
    let mut b = Bench::new("WEP_KEYBLADE_SO_03");
    let first = b.change();
    check("Hyper Hammer", &first);
    for _ in 0..600 {
        if b.step(&[]).state != StateId::Stylechange {
            break;
        }
    }
    for _ in 0..60 {
        b.step(&[]);
    }
    let second = b.change();
    check("Drill Punch", &second);
}
