//! (weapons P3) The form framework (documentation/KH3_WEAPONS_PLAN.md 1.3, package P3): KH3's formchanges as data
//! (`sheets/port/sora-keyblade-forms.json`) driven by one framework (`combat/form.rs`, `style.rs`).
//!
//! 1. THE GOLDEN TRACE: Second Form sessions recorded step by step BEFORE the framework replaced the hard-coded Second
//!    Form (`tests/golden/second_form_sessions.trace`); every run compares against it. A change in the trace is a change
//!    of Second Form. Re-record only on purpose: `KH_BLESS=1 cargo test -p kh-core --test form_framework`.
//! 2. The data-driven chain of a two-stage form at the framework level (Shooting Star: Double Arrowguns -> Magic
//!    Launcher), with a test moveset so the forms count as ready.
//! 3. The pending rule: a form without a moveset is never offered and its command is refused, never a crash.

use kh_core::test_world::TestWorld;
use kh_core::status::LoadoutValue;
use kh_core::{Input, Loadout, Output, PadButton, PadButtons, PadState, Pose, RecordingHost, Sora, Status, TargetFlags, TargetId, TargetInfo, Tuning, Vec3, SORA_PARAMS};
use std::fmt::Write as _;

const DT: f32 = 1.0 / 60.0;

// ------------------------------------------------------------------------------------ helpers (as combat2.rs)

fn cell(json: &serde_json::Value) -> Option<LoadoutValue> {
    match json {
        serde_json::Value::Number(n) => n.as_f64().map(LoadoutValue::Number),
        serde_json::Value::String(s) => Some(LoadoutValue::Text(s.clone())),
        serde_json::Value::Array(items) => items.iter().map(|i| i.as_str().map(str::to_string)).collect::<Option<Vec<_>>>().map(LoadoutValue::List),
        serde_json::Value::Object(map) => map.iter().map(|(k, v)| v.as_f64().map(|n| (k.clone(), n))).collect::<Option<Vec<_>>>().map(LoadoutValue::Map),
        _ => None,
    }
}

/// The level-99 loadout sheet with no equipped abilities.
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

fn dummy(id: u32, x: f32, y: f32, z: f32) -> TargetInfo {
    TargetInfo { id: TargetId(id), position: Vec3::new(x, y, z), center: Vec3::new(x, y, z + 90.0), velocity: Vec3::ZERO, yaw: 180.0, radius: 40.0, half_height: 90.0, flags: TargetFlags(TargetFlags::ALIVE | TargetFlags::HITTABLE | TargetFlags::LOCKABLE | TargetFlags::ENEMY), hp: 1.0e6, hp_max: 1.0e6 }
}

fn pad(held: &[PadButton]) -> PadState {
    let set = |list: &[PadButton]| list.iter().fold(PadButtons::NONE, |all, b| all.with(*b));
    PadState::from_held(set(held), PadButtons::NONE)
}

struct Bench {
    sora: Sora,
    host: RecordingHost,
    world: TestWorld,
    trace: String,
    steps: usize,
}

impl Bench {
    fn new(status: Status, dummies: &[TargetInfo]) -> Bench {
        let origin = Vec3::new(0.0, 0.0, Sora::origin_height_on_floor(&SORA_PARAMS));
        let sora = Sora::new(SORA_PARAMS, Pose { position: origin, yaw: 0.0 }).with_tuning(Tuning::GAME_EXACT).with_status(status);
        let mut host = RecordingHost::new();
        host.targets.extend_from_slice(dummies);
        let mut bench = Bench { sora, host, world: TestWorld::floor(), trace: String::new(), steps: 0 };
        for _ in 0..5 {
            bench.step(&[]);
        }
        bench
    }

    /// One step, appended to the trace: the state, the pose, the clip, the form's and the gauge's fields, the queue and
    /// the step's events. Only fields Second Form had before the framework are printed (new fields don't change it).
    fn step(&mut self, held: &[PadButton]) -> Output {
        let input = Input::default().with_pad(pad(held));
        let (sora, world) = (&mut self.sora, &self.world);
        let out = self.host.step(world, |h| sora.step_hosted(DT, &input, h));
        self.steps += 1;
        let c = self.sora.combat();
        let f = &c.form;
        let g = &c.style;
        let q = &c.situation;
        let s = self.sora.status();
        let _ = write!(
            self.trace,
            "{} {:?}/{} p={:?} y={:?} a={}#{} | form {} {} {:?}/{:?} fin {} {} {} {} fix {} pause {} up {} wear {} block {} cia {} slow {} {}:{}:{} | style {:?} {:?} {:?} {:?} {:?} {:?} {:?} {} | q {:?} a={:?} b={:?} x={:?} sel {} rm {} auto {} | hp {} ab {:?}",
            self.steps,
            out.state,
            out.sub_step,
            out.position,
            out.yaw,
            out.anim.name.name(),
            out.anim.play_id,
            f.cmd,
            f.level,
            f.time_left,
            f.time_full,
            f.finish_level,
            f.finish_cmd,
            f.award_pending,
            f.finish_pending,
            f.style_fix,
            f.paused,
            f.time_up,
            f.wear_form,
            f.bad_status_block,
            f.command_in_action,
            f.world_slow,
            f.begun,
            f.ended,
            f.nets,
            g.gauge,
            g.pending,
            g.chain,
            g.dec_start,
            g.dec_interval,
            g.points,
            g.last,
            g.awards,
            q.list.iter().map(|e| (e.kind, e.class, e.total, e.remaining, e.start_delay, e.appear, e.serial)).collect::<Vec<_>>(),
            q.slot_a.map(|e| (e.kind, e.total, e.remaining)),
            q.slot_b.map(|e| (e.kind, e.total, e.remaining)),
            q.extra,
            q.selected,
            q.removed,
            q.auto_finished,
            s.hp,
            [kh_core::enums::ETresAbilityKind::Highjump, kh_core::enums::ETresAbilityKind::ComboMaster, kh_core::enums::ETresAbilityKind::MpHaste].map(|k| s.abilities.count(k)),
        );
        if let Some(fv) = self.sora.finish() {
            let _ = write!(self.trace, " | finish {} {} {} {} {:?}", fv.cmd, fv.sub, fv.count, fv.pending, fv.root_len);
        }
        for e in &out.events {
            let _ = write!(self.trace, " | ev {:?}", e.kind);
        }
        self.trace.push('\n');
        out
    }

    fn tap(&mut self, button: PadButton) -> Output {
        self.step(&[button])
    }

    fn idle(&mut self, steps: usize) {
        for _ in 0..steps {
            self.step(&[]);
        }
    }

    fn mark(&mut self, what: &str) {
        let _ = writeln!(self.trace, "== {what}");
    }

    /// Fills the gauge, waits for the command and presses Triangle.
    fn formchange(&mut self) {
        self.sora.debug_add_style(60.0);
        self.idle(2);
        self.sora.debug_add_style(40.0);
        self.idle(15);
        self.tap(PadButton::SITUATION);
    }

    /// Triangle on every step while a finish extra is on show, until STYLEFINISH ends.
    fn run_finish(&mut self) {
        self.tap(PadButton::SITUATION);
        for _ in 0..1500 {
            let press = self.sora.combat().situation.extra.is_some();
            let out = if press { self.tap(PadButton::SITUATION) } else { self.step(&[]) };
            if out.state != kh_core::StateId::Stylefinish && self.sora.finish().is_none() {
                break;
            }
        }
    }
}

// ------------------------------------------------------------------------------------ 1. the golden trace

/// The Second Form sessions of the golden trace (as recorded before the framework).
fn second_form_sessions() -> String {
    let mut all = String::new();
    // A: the transformation, a ground combo in the form, two more full gauges (finish levels 2, 3), Last Arcanum with
    // every extra used, then idle after the end.
    {
        let mut b = Bench::new(level99(), &[dummy(1, 300.0, 0.0, 0.0), dummy(2, 0.0, 700.0, 0.0)]);
        b.mark("A: formchange");
        b.formchange();
        b.idle(200);
        b.mark("A: combo in the form");
        for _ in 0..240 {
            b.tap(PadButton::ATTACK);
        }
        b.idle(60);
        b.mark("A: finish levels 2 and 3");
        b.sora.debug_add_style(100.0);
        b.idle(3);
        b.sora.debug_add_style(100.0);
        b.idle(20);
        b.mark("A: Last Arcanum");
        b.run_finish();
        b.idle(60);
        all += &b.trace;
    }
    // B: Sonic Rave with every extra used.
    {
        let mut b = Bench::new(level99(), &[dummy(1, 300.0, 0.0, 0.0)]);
        b.mark("B: formchange");
        b.formchange();
        b.idle(200);
        b.sora.debug_add_style(100.0);
        b.idle(20);
        b.mark("B: Sonic Rave");
        b.run_finish();
        b.idle(30);
        all += &b.trace;
    }
    // C: Stun Impact at level 1.
    {
        let mut b = Bench::new(level99(), &[dummy(1, 300.0, 0.0, 0.0)]);
        b.formchange();
        b.idle(200);
        b.mark("C: Stun Impact");
        b.run_finish();
        b.idle(30);
        all += &b.trace;
    }
    // D: the form runs out (with a combo across its end), then the gauge decays and offers Second Form again.
    {
        let mut b = Bench::new(level99(), &[dummy(1, 200.0, 0.0, 0.0)]);
        b.formchange();
        b.idle(200);
        b.mark("D: run out during a combo");
        b.sora.combat_mut().form.time_left = 0.3;
        for _ in 0..120 {
            b.tap(PadButton::ATTACK);
        }
        b.idle(120);
        b.mark("D: the gauge again, the command expires");
        b.sora.debug_add_style(100.0);
        b.idle(40 * 60);
        all += &b.trace;
    }
    all
}

#[test]
fn second_form_golden_trace_is_unchanged() {
    let path = format!("{}/tests/golden/second_form_sessions.trace", env!("CARGO_MANIFEST_DIR"));
    let now = second_form_sessions();
    if std::env::var_os("KH_BLESS").is_some() || !std::path::Path::new(&path).exists() {
        std::fs::create_dir_all(std::path::Path::new(&path).parent().unwrap()).unwrap();
        std::fs::write(&path, &now).unwrap();
        eprintln!("recorded {path} ({} lines)", now.lines().count());
        return;
    }
    let golden = std::fs::read_to_string(&path).unwrap();
    if golden != now {
        let (line, (g, n)) = golden.lines().zip(now.lines()).enumerate().find(|(_, (g, n))| g != n).unwrap_or((golden.lines().count().min(now.lines().count()), ("<end>", "<end>")));
        panic!("Second Form changed: first difference at line {}\n golden: {g}\n    now: {n}\n(golden {} lines, now {} lines)", line + 1, golden.lines().count(), now.lines().count());
    }
}

// ------------------------------------------------------------------------------------ 2. a two-stage form, data-driven

fn ability(name: &str) -> kh_core::enums::ETresAbilityKind {
    kh_core::enums::ETresAbilityKind::from_game_name(name).unwrap()
}

fn holding(key: &str) -> Status {
    let mut s = level99();
    s.weapon = key.to_string();
    s
}

fn list(b: &Bench) -> Vec<(i32, f32)> {
    b.sora.combat().situation.list.iter().map(|e| (e.kind, e.total)).collect()
}

/// Steps until STYLECHANGE is over (at most `limit` steps); the number of steps it took.
fn through_the_transformation(b: &mut Bench, limit: usize) -> usize {
    for k in 1..=limit {
        if b.step(&[]).state != kh_core::StateId::Stylechange {
            return k;
        }
    }
    panic!("STYLECHANGE did not end in {limit} steps");
}

#[test]
fn shooting_star_double_arrowguns_leads_to_magic_launcher() {
    use kh_core::host::EventKind;
    use kh_core::enums::ETresCommandKind as Cmd;
    let mut b = Bench::new(holding("WEP_KEYBLADE_SO_02"), &[]);
    // The forms count as ready (no moveset yet: the framework's generic behaviour).
    b.sora.combat_mut().form.debug_ready = true;
    // A full gauge offers the first stage, CHANGE_DW, for its Param0 30 s.
    b.sora.debug_add_style(100.0);
    b.idle(15);
    assert_eq!(list(&b), [(0x80, 30.0)], "CHANGE_DW offered");
    let from = b.steps;
    b.tap(PadButton::SITUATION);
    assert_eq!(b.step(&[]).state, kh_core::StateId::Stylechange);
    let f = b.sora.combat().form;
    assert_eq!((f.cmd, f.level, f.time_full, f.grow), (0x80, 1, 20.0, 1), "Double Arrowguns: level 1, 20 s");
    through_the_transformation(&mut b, 200);
    // Its wear form (2, Wisdom) with p_ex001_FormAbility[TRES_WEAR_FORM_02]; no finish command (a first stage).
    let f = b.sora.combat().form;
    assert_eq!((f.wear_form, f.finish_pending, b.sora.combat().situation.slot_b.map(|e| e.kind)), (2, false, None));
    for name in ["HIGHJUMP", "DOUBLEFLIGHT", "LEAF_VEIL", "MP_HASTEGA", "MAGIC_DRAW"] {
        assert!(b.sora.status().abilities.has(ability(name)), "{name} in Double Arrowguns");
    }
    assert!(!b.sora.status().abilities.has(ability("MASTER_DRAW")), "not Second Form's list");
    assert_eq!(f.transform_clip(), Some("StyleChange2"));
    assert_eq!(f.projectile("DW_NORMAL"), Some("/Game/Blueprints/Projectile/DW/TresProjSS_Normal.TresProjSS_Normal_C"));
    assert!(f.weapon_units().iter().any(|u| u.starts_with("LWeapon@LEFT_HAND:L_wep1_skt=")), "two guns: {:?}", f.weapon_units());
    let events: Vec<String> = b.trace.lines().skip(from).collect::<Vec<_>>().join("\n").split(" | ev ").skip(1).map(|e| e.lines().next().unwrap_or("").to_string()).collect();
    assert!(events.iter().any(|e| e.starts_with("FormBegin { kind: ChangeDw, duration: 20.0 }")), "{events:?}");
    assert!(events.iter().any(|e| e.starts_with("WearForm { form: 2 }")), "{events:?}");
    // Inside the first stage a full gauge offers the second, CHANGE_BZ (form level 1 of weapon type 3).
    b.idle(30);
    b.sora.debug_add_style(100.0);
    b.idle(15);
    assert_eq!(list(&b), [(0x83, 30.0)], "CHANGE_BZ offered inside Double Arrowguns");
    b.tap(PadButton::SITUATION);
    assert_eq!(b.step(&[]).state, kh_core::StateId::Stylechange);
    through_the_transformation(&mut b, 200);
    // Magic Launcher: level 2, 20 s, the same wear form; its finish command FINISH_BZ in the finish slot.
    let f = b.sora.combat().form;
    assert_eq!((f.cmd, f.level, f.time_full, f.wear_form, f.finish_pending), (0x83, 2, 20.0, 2, true));
    assert_eq!(b.sora.combat().situation.slot_b.map(|e| e.kind), Some(0x9D), "FINISH_BZ");
    assert!(list(&b).is_empty(), "the used offer is gone");
    // A full gauge in the second stage: no further form, +3 s (never above the full 20), the finish command refreshed.
    b.idle(120);
    let t0 = b.sora.combat().form.time_left;
    b.sora.debug_add_style(100.0);
    b.step(&[]);
    let f = b.sora.combat().form;
    assert!((f.time_left - ((t0 + 3.0).min(20.0) - DT)).abs() < 0.01, "{} after {t0}", f.time_left);
    assert!(list(&b).is_empty() && b.sora.combat().situation.slot_b.map(|e| e.kind) == Some(0x9D));
    // Triangle on FINISH_BZ: STYLEFINISH, and the form ends with it. (F-ARROWGUN) Its body is now F-ARROWGUN's (slot 143:
    // M_5417, 3.0 s), no longer the at-once end of a finish without one.
    b.idle(15);
    b.tap(PadButton::SITUATION);
    b.idle(200);
    let f = b.sora.combat().form;
    assert_eq!((f.cmd, f.wear_form, f.finish_pending, f.ended, f.grow), (0, 0, false, 1, 0), "the finish ends the form");
    assert!(!b.sora.status().abilities.has(ability("DOUBLEFLIGHT")), "the form's abilities are gone");
    assert!(b.sora.combat().situation.slot_b.is_none());
    assert!(b.trace.contains("ev FormEnd { kind: ChangeBz }"));
    let _ = Cmd::ChangeBz;
    let _ = EventKind::FormEnd { kind: Cmd::ChangeBz };
}

#[test]
fn the_end_of_a_first_stage_deletes_its_second_stage_offer() {
    let mut b = Bench::new(holding("WEP_KEYBLADE_SO_02"), &[]);
    b.sora.combat_mut().form.debug_ready = true;
    b.formchange();
    through_the_transformation(&mut b, 200);
    b.sora.debug_add_style(100.0);
    b.idle(2);
    assert_eq!(list(&b), [(0x83, 30.0)]);
    // Double Arrowguns' 20 s run out: EndStyle (case 0x80) deletes CHANGE_BZ.
    b.idle(21 * 60);
    let f = b.sora.combat().form;
    assert_eq!((f.cmd, f.wear_form, f.ended), (0, 0, 1));
    assert!(list(&b).is_empty(), "CHANGE_BZ deleted with the first stage");
    assert!(b.trace.contains("ev SituationRemoved { kind: ChangeBz, why: Deleted }"));
}

// ------------------------------------------------------------------------------------ 3. the pending rule

#[test]
fn a_pending_form_is_not_offered_and_the_gauge_empties_as_kh3_s_does() {
    // A Keyblade whose first stage has no moveset: the full gauge is consumed (awards + 1), nothing offered.
    // (P3) Shooting Star made PENDING by the debug aid `Form::debug_pending` (its package has landed), so the rule is
    // tested however many form packages exist.
    let mut b = Bench::new(holding("WEP_KEYBLADE_SO_02"), &[]);
    b.sora.combat_mut().form.debug_pending = true;
    b.sora.debug_add_style(100.0);
    b.idle(20);
    let c = b.sora.combat();
    assert_eq!((c.style.gauge, c.style.awards, c.form.cmd), (0.0, 1, 0));
    assert!(list(&b).is_empty() && !b.trace.contains("SituationCommand"), "no formchange command");
    // The Second Form family stays ready under the aid (the framework's own moveset).
    let mut b = Bench::new(holding("WEP_KEYBLADE_SO_00"), &[]);
    b.sora.combat_mut().form.debug_pending = true;
    b.sora.debug_add_style(100.0);
    b.idle(2);
    assert_eq!(list(&b), [(0x94, 30.0)]);
    // Starlight and Dead of Night run Second Form's moveset: offered (CHANGE_L2 / CHANGE_L5).
    for (key, cmd) in [("WEP_KEYBLADE_SO_17", 0x95), ("WEP_KEYBLADE_SO_18", 0x98), ("WEP_KEYBLADE_SO_00", 0x94), ("", 0x94)] {
        let mut b = Bench::new(holding(key), &[]);
        b.sora.debug_add_style(100.0);
        b.idle(2);
        assert_eq!(list(&b), [(cmd, 30.0)], "{key}");
    }
    assert_ne!(kh_core::combat::forms::moveset_name("form.DW"), "-");
    assert_eq!(kh_core::combat::forms::moveset_name("form.L1"), "second_form");
}

// Dead of Night's Second Form D takes Second Form's whole path: the three clips, wear form 5, Stun Impact.
#[test]
fn second_form_d_runs_second_form() {
    let mut b = Bench::new(holding("WEP_KEYBLADE_SO_18"), &[]);
    b.formchange();
    through_the_transformation(&mut b, 200);
    let f = b.sora.combat().form;
    assert_eq!((f.cmd, f.level, f.wear_form, f.finish_cmd), (0x98, 1, 5, 171));
    assert!(b.trace.contains("a=StyleChangeEnd"));
    assert!(f.weapon_units()[0].ends_with("w_so121_Wep"), "Dead of Night's own Keyblade: {:?}", f.weapon_units());
}


// ------------------------------------------------------------------------------------ 4. a guard counter in a form (P3)

/// (P3) A guard counter in a form keeps the form's finish command usable: the Nano Counter in Nano Arms (FINISH_GM 0xA0),
/// the session F-NANO found the bug in. A prompt that takes slot B leaves the finish command where AwardAtMax's HUD query
/// (0x141621510) finds it (`Queue::hud_has`; KH3 shows the controller's prompts through the sub-command overlay
/// 0x14158C180 -> 0x14162A440 and never replaces slot B).
#[test]
fn a_guard_counter_in_a_form_keeps_its_finish_command_usable() {
    use kh_core::enums::ETresDamageKind;
    use kh_core::states::HitOnPlayer;
    let path = format!("{}/../../sheets/port/sora-loadout-99.json", env!("CARGO_MANIFEST_DIR"));
    let json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let mut loadout = Loadout::new("sora-loadout-99");
    for row in json["rows"].as_array().unwrap() {
        let k = row[0].as_str().unwrap();
        let value = if k == "abilities.equipped" { LoadoutValue::List(vec!["REFLECT_GUARD".to_string(), "GUARD_COUNTER".to_string()]) } else { cell(&row[1]).unwrap() };
        loadout.set(k, &value).unwrap();
    }
    loadout.resolve().unwrap();
    let mut status = Status::from_loadout(&loadout);
    status.weapon = "WEP_KEYBLADE_SO_08".to_string();
    let mut b = Bench::new(status, &[dummy(1, 200.0, 0.0, 0.0)]);
    b.sora.debug_add_style(100.0);
    b.idle(15);
    b.tap(PadButton::SITUATION);
    through_the_transformation(&mut b, 600);
    assert_eq!((b.sora.combat().form.cmd, b.sora.combat().form.finish_pending), (0x86, true), "Nano Arms with its finish given");
    assert_eq!(b.sora.combat().situation.slot_b.map(|e| e.kind), Some(0xA0), "FINISH_GM in slot B");
    b.idle(30);
    // The dummy right in front, a guard, a guarded hit: the counter window and its prompt.
    let out = b.step(&[]);
    let (me, fwd) = (out.position, Vec3::new(out.yaw.to_radians().cos(), out.yaw.to_radians().sin(), 0.0));
    let at = Vec3::new(me.x, me.y, 0.0) + fwd * 200.0;
    b.host.targets[0] = TargetInfo { position: at, center: at + Vec3::Z * 90.0, ..dummy(1, 0.0, 0.0, 0.0) };
    b.idle(20);
    b.step(&[PadButton::GUARD]);
    b.idle(10);
    let d = (Vec3::new(me.x, me.y, 0.0) - at).normalize_or_zero();
    b.sora.take_hit(HitOnPlayer { direction: d, causer_yaw: d.y.atan2(d.x).to_degrees(), ..HitOnPlayer::new(ETresDamageKind::Small, 30.0, TargetId(1), at) });
    b.idle(1);
    assert!(b.sora.actions().counter_window > 0.0, "the guard's counter window");
    // The counter (in this session Nano Arms' counter prompt SHIELDBURST, ex kind 14, takes slot A; the slot-B prompts,
    // ex kinds 1-3 / 5 / 9 / 13, are the unit test combat::form::tests::a_reaction_prompt_over_slot_b_keeps_the_finish):
    // the finish command stays given through it, and is used afterwards.
    for k in 0..121 {
        if k == 0 { b.tap(PadButton::ATTACK) } else { b.step(&[]) };
        assert!(b.sora.combat().form.finish_pending, "the finish command stays given (step {k})");
    }
    let f = b.sora.combat().form;
    assert_eq!((f.cmd, f.finish_pending), (0x86, true), "still in Nano Arms, the finish still given");
    assert_eq!(b.sora.combat().situation.slot_b.map(|e| e.kind), Some(0xA0), "FINISH_GM back in slot B");
    b.idle(15);
    b.tap(PadButton::SITUATION);
    b.step(&[]);
    assert_eq!(b.sora.finish().map(|v| v.cmd), Some(0xA0), "FINISH_GM runs (STYLEFINISH)");
}
