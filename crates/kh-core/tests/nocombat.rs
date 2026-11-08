//! (NOCOMBAT) THE USER (2026-11-03): "grey out all action commands except for item in the roundtable hold, you can't do
//! anything offensive there"; "this applies to all characters"; "you can't summon in round table hold either".
//! Sora's side of the no-combat rule (kh_core::nocombat) on the pad, with the gate on and off: the Attack press, the
//! Magic list, a shortcut, the shotlock hold, Y, Summon (its row and a host's request); Items stays usable; the menu
//! model greys every entry but Items.

use kh_core::command::summon::{self, AshSlot, SummonView};
use kh_core::command::{CommandRequest, MenuEvent, MenuState};
use kh_core::nocombat;
use kh_core::status::LoadoutValue;
use kh_core::test_world::TestWorld;
use kh_core::{Input, Loadout, Output, PadButton, PadButtons, PadState, Pose, RecordingHost, Sora, StateId, Status, Tuning, Vec3, SORA_PARAMS};

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

fn level99() -> Status {
    let path = format!("{}/../../sheets/port/sora-loadout-99.json", env!("CARGO_MANIFEST_DIR"));
    let json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))).unwrap();
    let mut loadout = Loadout::new("sora-loadout-99");
    for row in json["rows"].as_array().unwrap() {
        let key = row[0].as_str().unwrap();
        let value = if key == "abilities.equipped" { LoadoutValue::List(Vec::new()) } else { cell(&row[1]).unwrap() };
        loadout.set(key, &value).unwrap_or_else(|e| panic!("{e}"));
    }
    loadout.resolve().unwrap();
    Status::from_loadout(&loadout)
}

fn spirits() -> SummonView {
    let mut v = SummonView { allowed: true, bell: true, ..SummonView::default() };
    v.slots[0] = Some(AshSlot { goods: 213000, level: 0, name: "Ash".to_string(), mp: 11, renowned: false, mimic: false, usable: true, icon: None });
    v
}

struct Bench {
    sora: Sora,
    host: RecordingHost,
    world: TestWorld,
    held: PadButtons,
}

impl Bench {
    fn new() -> Bench {
        let origin = Vec3::new(0.0, 0.0, Sora::origin_height_on_floor(&SORA_PARAMS));
        let sora = Sora::new(SORA_PARAMS, Pose { position: origin, yaw: 0.0 }).with_tuning(Tuning::GAME_EXACT).with_status(level99());
        let mut b = Bench { sora, host: RecordingHost::new(), world: TestWorld::floor(), held: PadButtons::NONE };
        b.sora.command_mut().summon = spirits();
        b.run(10);
        b
    }

    fn step(&mut self, held: &[PadButton]) -> Output {
        let now = held.iter().fold(PadButtons::NONE, |all, b| all.with(*b));
        let pad = PadState::from_held(now, self.held);
        self.held = now;
        let input = Input::default().with_pad(pad);
        let (sora, world) = (&mut self.sora, &self.world);
        self.host.step(world, |h| sora.step_hosted(DT, &input, h))
    }

    fn run(&mut self, n: usize) -> Vec<Output> {
        (0..n).map(|_| self.step(&[])).collect()
    }

    /// A press: its frame's output and whether that frame refused it (KH3's refused SE), then the next frames.
    fn press(&mut self, b: PadButton) -> (bool, Vec<Output>) {
        let first = self.step(&[b]);
        let refused = self.sora.ui_snapshot().command.events.contains(&MenuEvent::Refused);
        let mut outs = vec![first];
        outs.extend(self.run(20));
        (refused, outs)
    }

    fn cursor_to(&mut self, row: usize) {
        while self.sora.ui_snapshot().command.main_cursor != row {
            self.press(PadButton::DPadDown);
        }
    }
}

fn reached(outs: &[Output], id: StateId) -> bool {
    outs.iter().any(|o| o.state == id)
}

#[test]
fn nocombat_attack_press_is_refused_in_the_zone_and_strikes_outside() {
    let mut b = Bench::new();
    {
        let _zone = nocombat::scope(true);
        let (refused, outs) = b.press(PadButton::ATTACK);
        assert!(refused, "A on the greyed Attack plays KH3's refused sound");
        assert!(!reached(&outs, StateId::Attack), "no swing in the zone");
    }
    let (refused, outs) = b.press(PadButton::ATTACK);
    assert!(!refused && reached(&outs, StateId::Attack), "outside the zone the same press attacks");
}

#[test]
fn nocombat_magic_list_and_a_host_request_are_refused() {
    let mut b = Bench::new();
    let _zone = nocombat::scope(true);
    b.cursor_to(1);
    let (refused, outs) = b.press(PadButton::ATTACK);
    assert!(refused, "Magic is greyed: its list does not open");
    assert_eq!(b.sora.ui_snapshot().command.state, MenuState::MainSel);
    assert!(!reached(&outs, StateId::Firemagic));
    let (refused, _) = b.press(PadButton::DPadRight);
    assert!(refused && b.sora.ui_snapshot().command.state == MenuState::MainSel, "D-pad right does not open it either");
    // a host's (AI's, a script's) command request goes through the same gate
    let fire = b.sora.command().magic_slots[0];
    b.sora.command_mut().requests.push(CommandRequest::Execute(fire));
    let outs = b.run(20);
    assert!(!reached(&outs, StateId::Firemagic), "a requested spell does not start");
}

#[test]
fn nocombat_shotlock_hold_never_opens_the_aim() {
    let mut b = Bench::new();
    let _zone = nocombat::scope(true);
    for _ in 0..60 {
        b.step(&[PadButton::RightShoulder]);
        assert_eq!(b.sora.aim().mode, 0, "the shotlock aim stays shut");
    }
    let _out = nocombat::scope(false);
    let opened = (0..60).any(|_| {
        b.step(&[PadButton::RightShoulder]);
        b.sora.aim().mode != 0
    });
    assert!(opened, "outside the zone the hold opens it");
}

#[test]
fn nocombat_summon_row_and_request_are_refused() {
    let mut b = Bench::new();
    let _zone = nocombat::scope(true);
    b.cursor_to(3);
    let snap = b.sora.ui_snapshot().command;
    assert_eq!(snap.main[3].name, summon::TEXT_SUMMON);
    assert!(!snap.main[3].enabled && snap.lists[3].iter().all(|r| !r.enabled), "Summon and every ash row greyed");
    let (refused, outs) = b.press(PadButton::ATTACK);
    assert!(refused && b.sora.ui_snapshot().command.state == MenuState::MainSel, "the list does not open: refused SE");
    assert!(!reached(&outs, StateId::PlayMotion));
    // the camp menu / a quick slot / any host asks with a request: refused the same way
    b.sora.command_mut().requests.push(CommandRequest::Execute(summon::KIND_SUMMON));
    b.step(&[]);
    assert!(b.sora.ui_snapshot().command.events.contains(&MenuEvent::Refused));
    let outs = b.run(30);
    assert!(!reached(&outs, StateId::PlayMotion), "no chant, no spirit");
    assert_eq!(b.sora.status().mp, b.sora.status().max_mp(), "nothing was paid");
}

#[test]
fn nocombat_shortcut_spell_refused_item_shortcut_used() {
    let mut b = Bench::new();
    let _zone = nocombat::scope(true);
    let page = b.sora.command().shortcut_page as usize;
    let slots = b.sora.command().shortcuts[page];
    let faces = [PadButton::FaceRight, PadButton::FaceTop, PadButton::FaceLeft, PadButton::FaceBottom];
    let spell = slots.iter().position(|k| *k != 0 && kh_core::magic::items::item_of_command(*k).is_none());
    if let Some(slot) = spell {
        b.step(&[PadButton::LeftShoulder]);
        b.step(&[PadButton::LeftShoulder, faces[slot]]);
        let refused = b.sora.ui_snapshot().command.events.contains(&MenuEvent::Refused);
        let outs: Vec<Output> = (0..20).map(|_| b.step(&[PadButton::LeftShoulder])).collect();
        assert!(refused && !reached(&outs, StateId::Firemagic), "a spell shortcut is refused aloud");
        b.run(5);
    }
    // an item on the panel stays usable
    b.sora.command_mut().shortcuts[page][3] = kh_core::magic::items::command_of_item(kh_core::status::ItemKind::Potion).unwrap_or(0);
    b.step(&[PadButton::LeftShoulder]);
    let mut outs = vec![b.step(&[PadButton::LeftShoulder, PadButton::FaceBottom])];
    outs.extend((0..40).map(|_| b.step(&[PadButton::LeftShoulder])));
    assert!(reached(&outs, StateId::UseItem), "the Potion on the panel is used");
}

#[test]
fn nocombat_items_stay_usable_from_the_menu() {
    let mut b = Bench::new();
    let _zone = nocombat::scope(true);
    b.sora.status_mut().hp -= 10;
    b.cursor_to(2);
    let snap = b.sora.ui_snapshot().command;
    assert!(snap.main[2].enabled && snap.lists[2].iter().any(|r| r.enabled), "Items and its rows stay lit");
    let (refused, _) = b.press(PadButton::ATTACK);
    assert!(!refused);
    assert_eq!(b.sora.ui_snapshot().command.state, MenuState::SubSel, "the Items list opens");
    let (refused, outs) = b.press(PadButton::ATTACK);
    assert!(!refused && reached(&outs, StateId::UseItem), "the item is used");
}

#[test]
fn nocombat_y_jump_guard_and_moving_stay() {
    let mut b = Bench::new();
    let _zone = nocombat::scope(true);
    let (refused, _) = b.press(PadButton::SITUATION);
    assert!(!refused, "Y with nothing on the Y list is silent");
    let (_, outs) = b.press(PadButton::JUMP);
    assert!(reached(&outs, StateId::Jump), "the jump stays");
    // X (guard / dodge) does in the zone exactly what it does outside it
    let guard = |zone: bool| {
        let _rule = nocombat::scope(zone);
        let mut b = Bench::new();
        let mut states = vec![b.step(&[PadButton::GUARD]).state];
        states.extend((0..20).map(|_| b.step(&[PadButton::GUARD]).state));
        states
    };
    assert_eq!(guard(true), guard(false), "the guard press is untouched");
}

#[test]
fn nocombat_menu_model_greys_every_entry_but_items() {
    let mut b = Bench::new();
    let off = b.sora.ui_snapshot().command;
    assert!(off.main[0].enabled && off.main[1].enabled, "outside the zone Attack and Magic are lit");
    let _zone = nocombat::scope(true);
    b.run(2);
    let on = b.sora.ui_snapshot().command;
    assert!(!on.main[0].enabled && !on.main[1].enabled && !on.main[3].enabled, "Attack, Magic, Summon greyed");
    assert!(on.main[2].enabled, "Items lit");
    assert!(on.lists[1].iter().all(|r| !r.enabled), "every spell greyed");
    assert!(on.lists[2].iter().all(|r| r.enabled == off.lists[2].iter().find(|o| o.kind == r.kind).is_some_and(|o| o.enabled)), "the item rows as before");
    for r in on.shortcut.iter() {
        assert!(!r.enabled || kh_core::magic::items::item_of_command(r.kind).is_some(), "a lit shortcut is an item: {r:?}");
    }
}
