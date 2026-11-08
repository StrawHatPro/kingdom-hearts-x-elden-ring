//! (THE USER 2026-11-02: "Give Sora the ability to 'Summon' from the command menu, but he uses Elden Ring's spirit ashes
//! ... Summon is just greyed out if nothing is equipped. Summoning is also only available when you get the spirit calling
//! bell.") THE SUMMON ROW on the pad, as KH3's fourth main command: the greying rules (no bell, nothing equipped, MP
//! Charge, the chant), the list of the equipped ashes, the MP payment (an overspend starts MP Charge), the chant (KH3's
//! Link call M_1571) and its events (Chant, then Spawn at the call), "Unsummon" and its refund (THE USER 2026-11-02:
//! "if 100% of the timer remained then Sora would get back 100% MP").

use kh_core::command::summon::{self, AshSlot, SpiritOut, SummonPhase, SummonView};
use kh_core::command::MenuEvent;
use kh_core::host::EventKind;
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

fn ash(goods: u32, mp: i32) -> AshSlot {
    AshSlot { goods, level: summon::level_of(goods), name: format!("Ash {goods}"), mp, renowned: false, mimic: false, usable: true, icon: None }
}

struct Bench {
    sora: Sora,
    host: RecordingHost,
    world: TestWorld,
    held: PadButtons,
}

impl Bench {
    fn new(view: SummonView) -> Bench {
        let origin = Vec3::new(0.0, 0.0, Sora::origin_height_on_floor(&SORA_PARAMS));
        let sora = Sora::new(SORA_PARAMS, Pose { position: origin, yaw: 0.0 }).with_tuning(Tuning::GAME_EXACT).with_status(level99());
        let mut b = Bench { sora, host: RecordingHost::new(), world: TestWorld::floor(), held: PadButtons::NONE };
        b.sora.command_mut().summon = view;
        for _ in 0..10 {
            b.step(&[]);
        }
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

    /// A press (one frame down, one up): the outputs of both frames.
    fn press(&mut self, b: PadButton) -> Vec<Output> {
        vec![self.step(&[b]), self.step(&[])]
    }

    /// The main cursor to the fourth row (D-pad down three times from Attack).
    fn goto_summon_row(&mut self) {
        for _ in 0..3 {
            self.press(PadButton::DPadDown);
        }
        assert_eq!(self.sora.ui_snapshot().command.main_cursor, 3);
    }

    fn row(&self) -> kh_core::command::CommandRow {
        self.sora.ui_snapshot().command.main[3]
    }
}

/// The latest step refused a press (KH3's refused sound).
fn refused(b: &Bench) -> bool {
    b.sora.ui_snapshot().command.events.contains(&MenuEvent::Refused)
}

fn summon_events(outs: &[Output]) -> Vec<(SummonPhase, u8, i32)> {
    outs.iter().flat_map(|o| o.events.iter()).filter_map(|e| if let EventKind::Summon { phase, slot, mp } = e.kind { Some((phase, slot, mp)) } else { None }).collect()
}

fn ready() -> SummonView {
    let mut v = SummonView { allowed: true, bell: true, ..SummonView::default() };
    v.slots[0] = Some(ash(213000, 11));
    v.slots[2] = Some(ash(200003, 66));
    v
}

#[test]
fn summon_no_row_without_a_host_that_has_spirits() {
    let b = Bench::new(SummonView::default());
    let c = b.sora.ui_snapshot().command;
    assert!(c.link_none, "AI Sora / Mimic Sora / a host without spirits: KH3's three rows, as before");
    assert_eq!(c.main[3].name, "Link");
}

#[test]
fn summon_greyed_without_the_bell_or_with_nothing_equipped() {
    let mut no_bell = ready();
    no_bell.bell = false;
    let mut b = Bench::new(no_bell);
    let row = b.row();
    assert!(!b.sora.ui_snapshot().command.link_none, "the fourth row is shown");
    assert_eq!((row.name, row.enabled), ("Summon", false), "THE USER: only with the Spirit Calling Bell");
    b.goto_summon_row();
    let outs = b.press(PadButton::ATTACK);
    assert!(summon_events(&outs).is_empty());
    let empty = SummonView { allowed: true, bell: true, ..SummonView::default() };
    let b = Bench::new(empty);
    assert_eq!((b.row().name, b.row().enabled), ("Summon", false), "THE USER: greyed if nothing is equipped");
}

#[test]
fn summon_list_pays_the_mp_plays_the_chant_and_calls_the_spirit() {
    let mut b = Bench::new(ready());
    assert!(b.row().enabled);
    let mp0 = b.sora.status().mp;
    b.goto_summon_row();
    b.press(PadButton::ATTACK);
    let c = b.sora.ui_snapshot().command;
    assert_eq!(c.list, 3, "the Summon list is open");
    assert_eq!(c.lists[3].iter().map(|r| r.kind).collect::<Vec<_>>(), vec![summon::KIND_SUMMON, summon::KIND_SUMMON + 2], "the equipped slots, in order");
    let mut outs = b.press(PadButton::ATTACK);
    let paid = b.sora.status().effective_mp_cost(11, 0);
    assert_eq!(b.sora.status().mp, mp0 - paid, "the cost was paid");
    assert_eq!(b.sora.output().state, StateId::PlayMotion, "the chant state");
    assert_eq!(summon_events(&outs), vec![(SummonPhase::Chant, 0, paid)]);
    assert!(b.sora.command().calling);
    let row = b.row();
    assert!(!row.enabled, "no second summon while calling");
    for _ in 0..240 {
        outs.push(b.step(&[]));
    }
    let ev = summon_events(&outs);
    assert_eq!(ev.last(), Some(&(SummonPhase::Spawn, 0, paid)), "the call came: the host summons slot 0");
    assert_eq!(ev.iter().filter(|e| e.0 == SummonPhase::Spawn).count(), 1, "once");
    assert!(!b.sora.command().calling, "the chant ended");
    assert_ne!(b.sora.output().state, StateId::PlayMotion);
}

#[test]
fn summon_an_overspend_empties_the_bar_into_mp_charge_and_charge_greys_the_row() {
    let mut b = Bench::new(ready());
    b.sora.status_mut().mp = 10;
    b.goto_summon_row();
    b.press(PadButton::ATTACK);
    b.press(PadButton::DPadDown); // to slot 2 (66 MP)
    let c = b.sora.ui_snapshot().command;
    assert!(c.lists[3][1].mp_short && c.lists[3][1].enabled, "short of MP: recoloured, still callable (KH3's col2)");
    b.press(PadButton::ATTACK);
    assert_eq!(b.sora.status().mp, 0);
    assert!(b.sora.status().mp_charging, "KH3's UseMagicPoint: the overspend starts MP Charge");
    for _ in 0..240 {
        b.step(&[]);
    }
    let row = b.row();
    assert_eq!((row.name, row.enabled), ("Summon", false), "MP Charge greys Summon");
    b.goto_summon_row_anywhere();
    b.press(PadButton::ATTACK);
    assert!(refused(&b) || b.sora.ui_snapshot().command.list != 3, "refused, the list stays shut");
}

impl Bench {
    fn goto_summon_row_anywhere(&mut self) {
        while self.sora.ui_snapshot().command.main_cursor != 3 {
            self.press(PadButton::DPadDown);
        }
    }
}

#[test]
fn summon_unsummon_refunds_the_share_of_the_time_left() {
    for (left, want) in [(60.0, 40), (30.0, 20), (0.0, 0)] {
        let mut view = ready();
        view.out = Some(SpiritOut { slot: 2, name: "Ash".into(), cost: 40, left, full: 60.0, hp: 500, hp_max: 500, mimic: false, icon: None });
        let mut b = Bench::new(view);
        b.sora.status_mut().mp = 50;
        let row = b.row();
        assert_eq!((row.name, row.enabled, row.kind), ("Unsummon", true, summon::KIND_UNSUMMON), "THE USER: the row reads Unsummon while a spirit is out");
        b.goto_summon_row();
        let outs = b.press(PadButton::ATTACK);
        assert_eq!(summon_events(&outs), vec![(SummonPhase::Unsummon, 2, want)], "{left} s of 60 left");
        assert_eq!(b.sora.status().mp, (50 + want).min(b.sora.status().max_mp()));
        assert_eq!(b.row().name, "Summon", "the row swaps back at once");
    }
}

#[test]
fn summon_a_refund_during_mp_charge_shortens_the_charge() {
    let mut view = ready();
    view.out = Some(SpiritOut { slot: 0, name: "Ash".into(), cost: 60, left: 30.0, full: 60.0, hp: 1, hp_max: 1, mimic: false, icon: None });
    let mut b = Bench::new(view);
    b.sora.status_mut().force_mp_zero();
    let before = b.sora.status().mp_charge_timer;
    let row = b.row();
    assert!(row.enabled, "Unsummon stays lit in MP Charge (it gives MP back)");
    b.goto_summon_row();
    let outs = b.press(PadButton::ATTACK);
    assert_eq!(summon_events(&outs), vec![(SummonPhase::Unsummon, 0, 30)]);
    let s = b.sora.status();
    assert!(s.mp_charging, "30 MP of a larger bar does not end the charge");
    let expect = 30.0 / s.max_mp() as f32 * s.mp_charge_time();
    assert!((before - s.mp_charge_timer - expect).abs() < 0.2, "the charge lost {} s, expected about {expect}", before - s.mp_charge_timer);
    // a refund worth the whole bar ends it with full MP
    let mut st = level99();
    st.force_mp_zero();
    let max = st.max_mp();
    assert_eq!(summon::give_refund(&mut st, max), summon::Refunded::Charge { units: max, ended: true });
    assert!(!st.mp_charging && st.mp == max);
}

/// (THE USER 2026-11-03: "mimic tear should make Sora instantly go into MP recharge") the Mimic Tear empties his MP and
/// starts MP Charge whatever he had; it is refused while MP Charge runs; its refund base is the MP he had.
#[test]
fn summon_the_mimic_tear_takes_all_mp_into_mp_charge() {
    let mut view = SummonView { allowed: true, bell: true, ..SummonView::default() };
    view.slots[1] = Some(AshSlot { mimic: true, renowned: true, ..ash(207000, 99) });
    let mut b = Bench::new(view.clone());
    b.sora.status_mut().mp = 37;
    let c = b.sora.ui_snapshot().command;
    assert!(c.lists[3][0].enabled && !c.lists[3][0].mp_short, "an all-MP ash is never short");
    b.goto_summon_row();
    b.press(PadButton::ATTACK);
    let outs = b.press(PadButton::ATTACK);
    assert_eq!(summon_events(&outs), vec![(SummonPhase::Chant, 1, 37)], "the refund base: the 37 MP he had");
    assert_eq!(b.sora.status().mp, 0);
    assert!(b.sora.status().mp_charging, "MP Charge at once");
    // in MP Charge it is refused
    let mut b = Bench::new(view);
    b.sora.status_mut().force_mp_zero();
    b.step(&[]);
    assert!(!b.row().enabled, "MP Charge greys Summon");
    b.goto_summon_row();
    let outs = b.press(PadButton::ATTACK);
    assert!(summon_events(&outs).is_empty() && b.sora.ui_snapshot().command.list != 3);
    assert!(summon::takes_all(&AshSlot { mimic: true, ..AshSlot::default() }) && !summon::takes_all(&ash(213000, 11)));
}
