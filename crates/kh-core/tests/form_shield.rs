//! (weapons F-SHIELD) Counter Shield (Hero's Origin, CHANGE_SH 0x89) and Frying Pan (Grand Chef, CHANGE_SH2 0x91): the
//! trace of each form from the offer to its end (build/sora_tmp/weapons/F-SHIELD/STATE.md):
//! offer -> STYLECHANGE (StyleChange11 / StyleChange19, the wear form, the weapon at grow LV1) -> the COMBO_SH combo
//! (0x50 0x51 0x52 (+ M_B414 / M_B415) 0x53 0x54) -> a grown shield's attack (0x55 / 0x56, invincible, back to LV1) ->
//! FINISH_SH (Pegasus's ride with its thunder extra / the Frying Pan) -> the form's end.

use kh_core::host::recording::Call;
use kh_core::host::EventKind;
use kh_core::states::HitOnPlayer;
use kh_core::enums::ETresDamageKind;
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
        let value = if k == "abilities.equipped" { LoadoutValue::List(vec!["REFLECT_GUARD".to_string()]) } else { cell(&row[1]).unwrap() };
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
    /// Every step: (state, sub-step, clip, the events).
    log: Vec<(StateId, u8, &'static str, Vec<EventKind>)>,
    held: PadButtons,
}

impl Bench {
    fn new(key: &str) -> Bench {
        let origin = Vec3::new(0.0, 0.0, Sora::origin_height_on_floor(&SORA_PARAMS));
        let sora = Sora::new(SORA_PARAMS, Pose { position: origin, yaw: 0.0 }).with_tuning(Tuning::GAME_EXACT).with_status(holding(key));
        let mut host = RecordingHost::new();
        host.targets.push(dummy(1, 200.0));
        let mut b = Bench { sora, host, world: TestWorld::floor(), log: Vec::new(), held: PadButtons::NONE };
        b.idle(5);
        b
    }

    fn step(&mut self, held: &[PadButton]) -> Output {
        self.step_stick(held, kh_core::Vec2::ZERO)
    }

    fn step_stick(&mut self, held: &[PadButton], stick: kh_core::Vec2) -> Output {
        let set = held.iter().fold(PadButtons::NONE, |all, b| all.with(*b));
        let before = self.held;
        self.held = set;
        let mut input = Input::default().with_pad(PadState::from_held(set, before));
        input.move_stick = stick;
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

    fn events(&self, from: usize) -> Vec<String> {
        self.log[from..].iter().flat_map(|(_, _, _, e)| e.iter().map(|k| format!("{k:?}"))).collect()
    }

    /// The attack rows the dummy was hit with since call number from of the host.
    fn hit_rows(&self, from: usize) -> Vec<&'static str> {
        self.host.calls[from..].iter().filter_map(|c| match c { Call::Hit(h, r) if r.accepted => Some(h.attack_name), _ => None }).collect()
    }

    fn calls(&self) -> usize {
        self.host.calls.len()
    }

    /// The offer, Triangle, the transformation; returns the step the form began at.
    fn transform(&mut self, cmd: i32) -> usize {
        self.sora.debug_add_style(100.0);
        self.idle(15);
        let list: Vec<(i32, f32)> = self.sora.combat().situation.list.iter().map(|e| (e.kind, e.total)).collect();
        assert_eq!(list, [(cmd, 30.0)], "the formchange is offered for its 30 s");
        let from = self.log.len();
        self.step(&[PadButton::SITUATION]);
        assert_eq!(self.step(&[]).state, StateId::Stylechange);
        for _ in 0..400 {
            if self.step(&[]).state != StateId::Stylechange {
                return from;
            }
        }
        panic!("STYLECHANGE did not end");
    }

    /// Attack presses until the chain ends (back to IDLE); the kinds played.
    fn combo(&mut self) -> Vec<u8> {
        let first = self.sora.combat().history.len();
        for k in 0..900 {
            let out = if k % 2 == 0 { self.step(&[PadButton::ATTACK]) } else { self.step(&[]) };
            if k > 30 && matches!(out.state, StateId::Idle | StateId::Run | StateId::Jump) {
                break;
            }
        }
        self.sora.combat().history[first..].to_vec()
    }

    /// Triangle on the finish command, then on every step its extra is on show, until STYLEFINISH ends; the sub-steps.
    fn finish(&mut self) -> Vec<u8> {
        self.step(&[PadButton::SITUATION]);
        let mut subs = Vec::new();
        for _ in 0..3000 {
            let press = self.sora.combat().situation.extra.is_some() && self.log.len().is_multiple_of(2); // (F-SHAPES) Clippy manual_is_multiple_of
            let out = if press { self.step(&[PadButton::SITUATION]) } else { self.step(&[]) };
            match self.sora.finish() {
                Some(f) if subs.last() != Some(&f.sub) => subs.push(f.sub),
                Some(_) => {}
                None if !matches!(out.state, StateId::Stylefinish | StateId::StylefinishSh) => break,
                None => {}
            }
        }
        subs
    }
}

#[test]
fn counter_shield_from_the_offer_to_its_end() {
    let mut b = Bench::new("WEP_KEYBLADE_SO_01");
    assert_eq!(kh_core::combat::forms::moveset_name("form.SH"), "shield");
    // ---- offer -> transform
    let from = b.transform(0x89);
    let f = b.sora.combat().form;
    assert_eq!((f.cmd, f.level, f.time_full, f.wear_form, f.grow), (0x89, 1, 30.0, 3, 1), "Counter Shield: level 1, 30 s, wear form 3 (Solid), LV1");
    assert!(b.clips(from).contains(&"StyleChange11"), "{:?}", b.clips(from));
    assert!(b.events(from).iter().any(|e| e == "VoiceGroupNo { number: 2 }"), "StyleChange +0xA4 = 2: the voice group");
    assert!(f.weapon_units()[0].ends_with("w_so032_wep"), "{:?}", f.weapon_units());
    assert_eq!(b.sora.combat().situation.slot_b.map(|e| e.kind), Some(0xA3), "FINISH_SH on offer");
    // ---- the combo: COMBO_SH, 0x50 0x51 0x52 (the raid M_B413 -> M_B414 -> M_B415) 0x53 0x54
    b.idle(30);
    let from = b.log.len();
    let calls = b.calls();
    let kinds = b.combo();
    assert_eq!(kinds, [0x50, 0x51, 0x52, 0x53, 0x54], "the ground combo near one target");
    // The shield's own hit shapes (ATK_01 of w_so032 per clip frame) land with Counter Shield's rows.
    let rows = b.hit_rows(calls);
    assert!(rows.iter().any(|r| r.starts_with("SCSH_")) && !rows.iter().any(|r| r.starts_with("SH2_")), "{rows:?}");
    assert!(b.log[from..].iter().any(|l| l.0 == StateId::ComboSh), "the attack class is COMBO_SH");
    let clips = b.clips(from);
    for c in ["B419", "B412", "B413", "B416", "B417"] {
        assert!(clips.contains(&c), "{c} not played: {clips:?}");
    }
    // The raid (slot 125): with the next attack queued M_B413 chains at its end (M_B414 is played and left at once, as
    // "if +0x143: m_bEnableChange" does); without one it runs M_B413 -> M_B414 -> M_B415.
    b.idle(60);
    // (F-SHAPES) B417's SH_Finish / SH_Finish2 are their real tres_colltest01 hulls now (radius 600 and up to 600, not spheres of
    // 75): they reach the dummy (SCSH_FINISH / SCSH_FINISH2 hit), the combo ends on another step and its last press starts one
    // more attack (0x52, the raid). Let that chain end before the raid is traced.
    for _ in 0..600 {
        if b.step(&[]).state != StateId::ComboSh {
            break;
        }
    }
    let from = b.log.len();
    for _ in 0..600 {
        if b.sora.combat().history.last() == Some(&0x52) && b.log.last().is_some_and(|l| l.0 == StateId::ComboSh) {
            break;
        }
        b.step(&[PadButton::ATTACK]);
        b.step(&[]);
    }
    b.idle(120);
    let clips = b.clips(from);
    let at = |c: &str| clips.iter().position(|x| *x == c);
    assert!(at("B413") < at("B414") && at("B414") < at("B415") && at("B413").is_some(), "the raid chain: {clips:?}");
    // ---- a grown shield: its attack 0x55 (invincible, the world slowed) and the weapon back to LV1
    b.idle(30);
    b.sora.combat_mut().form.grow = 2;
    assert!(b.sora.combat().form.weapon_units().iter().any(|u| u.ends_with("w_so033_wep")), "LV2's parts");
    let from = b.log.len();
    b.step(&[PadButton::ATTACK]);
    b.step(&[]);
    assert_eq!(b.sora.combat().history.last(), Some(&0x55), "Wrathful Fist");
    assert!(b.sora.status().damage_invalid, "invincible during it");
    assert!(b.sora.combat().form.world_slow);
    b.idle(200);
    assert_eq!(b.sora.combat().form.grow, 1, "spent: LV1 again");
    assert!(!b.sora.status().damage_invalid && !b.sora.combat().form.world_slow);
    assert!(b.clips(from).contains(&"B418"));
    // ---- FINISH_SH: Pegasus's ride, the thunder extra, the landing; the form ends
    let shots = b.sora.combat().shots_spawned;
    let from = b.log.len();
    let subs = b.finish();
    assert_eq!(subs, [1, 2, 3, 4, 5, 6, 7], "StyleFinishSH Tick's sub-steps");
    let clips = b.clips(from);
    for c in ["B423", "B424", "M_1501"] {
        assert!(clips.contains(&c), "{c}: {clips:?}");
    }
    assert!(b.events(from).iter().any(|e| e.contains("SituationExtra") || e.contains("Extra")) || b.sora.combat().shots_spawned > shots, "the ride's extra / shots");
    assert!(b.sora.combat().shots_spawned >= shots + 3, "the rush and the thunders (one a second): {} -> {}", shots, b.sora.combat().shots_spawned);
    assert!(b.log[from..].iter().any(|l| l.0 == StateId::StylefinishSh), "FINISH_SH runs as STYLEFINISH_SH (38)");
    b.idle(5);
    let f = b.sora.combat().form;
    assert_eq!((f.cmd, f.wear_form, f.grow), (0, 0, 0), "the finish ends the form");
}

#[test]
fn frying_pan_from_the_offer_to_its_end() {
    let mut b = Bench::new("WEP_KEYBLADE_SO_10");
    let from = b.transform(0x91);
    let f = b.sora.combat().form;
    assert_eq!((f.cmd, f.wear_form, f.grow), (0x91, 4, 1), "Frying Pan: wear form 4 (Grace)");
    assert!(b.clips(from).contains(&"StyleChange19"));
    assert!(!b.events(from).iter().any(|e| e == "VoiceGroupNo { number: 2 }"), "only Counter Shield's transformation has voice group 2");
    assert!(f.weapon_units()[0].ends_with("w_so142_Wep"));
    b.idle(30);
    let from = b.log.len();
    let calls = b.calls();
    assert_eq!(b.combo(), [0x50, 0x51, 0x52, 0x53, 0x54], "the same combo");
    // ... hitting with Frying Pan's rows (0x141357590: SCSH_* -> SH2_*).
    let rows = b.hit_rows(calls);
    assert!(rows.iter().any(|r| r.starts_with("SH2_")) && !rows.iter().any(|r| r.starts_with("SCSH_")), "{rows:?}");
    // Its fire effects (effect group 0x867 of 0x50) instead of Counter Shield's lightning.
    let ev = b.events(from);
    assert!(ev.iter().any(|e| e.contains("vw_so140_loc01")), "Frying Pan's own effect");
    assert!(!ev.iter().any(|e| e.contains("vw_so032_atk1")), "not Counter Shield's");
    // LV3: Flame Barrage (0x56).
    b.idle(90);
    b.sora.combat_mut().form.grow = 3;
    b.step(&[PadButton::ATTACK]);
    b.step(&[]);
    assert_eq!(b.sora.combat().history.last(), Some(&0x56));
    b.idle(300);
    assert_eq!(b.sora.combat().form.grow, 1);
    // FINISH_SH: the Frying Pan (its hit at M_J423's window), then the end.
    let shots = b.sora.combat().shots_spawned;
    let from = b.log.len();
    let subs = b.finish();
    assert_eq!(subs, [2, 4, 5, 6], "slot 167's sub-steps");
    assert!(b.clips(from).contains(&"J423"));
    assert_eq!(b.sora.combat().shots_spawned, shots + 1, "the pan's hit");
    b.idle(5);
    assert_eq!(b.sora.combat().form.cmd, 0);
}

/// A hit from 150 cm in front of him (he faces +X at the start).
fn hit_in_front(own: Vec3) -> HitOnPlayer {
    HitOnPlayer { direction: Vec3::new(-1.0, 0.0, 0.0), causer_yaw: 180.0, ..HitOnPlayer::new(ETresDamageKind::Small, 30.0, TargetId(1), own + Vec3::new(150.0, 0.0, 0.0)) }
}

// The shield guard (Guard slot 118): held as long as the button is, the shield grows on each successful block (GuardChange
// at LV1, GuardChange2 at LV2, GrowUp), it walks with the stick (GuardWalk), and the grown shield's next attack is the
// grown attack.
#[test]
fn counter_shield_guard_grows_walks_and_the_attack_spends_it() {
    let mut b = Bench::new("WEP_KEYBLADE_SO_01");
    b.transform(0x89);
    b.idle(30);
    let from = b.log.len();
    for _ in 0..40 {
        b.step(&[PadButton::GUARD]);
    }
    assert_eq!(b.log.last().map(|l| l.0), Some(StateId::Guard), "held: still guarding");
    let clips = b.clips(from);
    assert!(clips.contains(&"B287") && clips.contains(&"B288"), "GuardStart -> GuardLoop of AnimSet11: {clips:?}");
    // The first block: GuardChange and LV2.
    let own = b.sora.output().position;
    b.sora.take_hit(hit_in_front(own));
    let hp = b.sora.status().hp;
    for _ in 0..3 {
        b.step(&[PadButton::GUARD]);
    }
    assert_eq!(b.sora.combat().form.grow, 2, "GrowUp");
    assert_eq!(b.sora.status().hp, hp, "guarded");
    assert!(b.clips(from).contains(&"B285"), "GuardChange: {:?}", b.clips(from));
    // Held through the clip: back to the guard, then the second block: GuardChange2, LV3.
    for _ in 0..110 {
        b.step(&[PadButton::GUARD]);
    }
    let own = b.sora.output().position;
    b.sora.take_hit(hit_in_front(own));
    for _ in 0..3 {
        b.step(&[PadButton::GUARD]);
    }
    assert_eq!(b.sora.combat().form.grow, 3);
    assert!(b.clips(from).contains(&"B286"), "GuardChange2");
    for _ in 0..110 {
        b.step(&[PadButton::GUARD]);
    }
    // The walk: guard held and the stick forward.
    let before = b.sora.output().position;
    for _ in 0..40 {
        b.step_stick(&[PadButton::GUARD], kh_core::Vec2::new(0.0, 1.0));
    }
    let moved = (b.sora.output().position - before).truncate().length();
    assert!(moved > 30.0, "the guard walks: {moved}");
    assert!(b.clips(from).iter().any(|c| c.starts_with("B427")) || b.clips(from).contains(&"GuardWalk"), "GuardWalk: {:?}", b.clips(from));
    // Released: GuardEnd, then the grown shield's attack: Wrathful Flurry (0x56), back to LV1.
    b.idle(90);
    assert!(b.clips(from).contains(&"B289"), "GuardEnd");
    b.step(&[PadButton::ATTACK]);
    b.step(&[]);
    assert_eq!(b.sora.combat().history.last(), Some(&0x56));
    b.idle(250);
    assert_eq!(b.sora.combat().form.grow, 1);
}
