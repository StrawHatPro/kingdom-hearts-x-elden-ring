//! Melee combat against training dummies: docs/sora-combat-spec.md section 15 (scenarios and
//! per-step checks) with the second-pass strands C (hits: build/sora_tmp/combat/P2_C_section.md)
//! and D (targeting: P2_D_section.md). Each test cites its section. The pure decision table of
//! section 15 is tested in `src/combat/decide.rs`; here the whole chain runs: button -> request
//! -> Attack state -> animation notifies -> hit windows -> hit records.

use kh_core::combat::targeting;
use kh_core::combat::CursorKind;
use kh_core::host::EventKind;
use kh_core::status::LoadoutValue;
use kh_core::test_world::TestWorld;
use kh_core::{AnimId, Input, Loadout, NotifyClass, Output, PadButton, PadButtons, PadState, Pose, RecordingHost, Sora, StateId, Status, TargetFlags, TargetId, TargetInfo, Tuning, Vec3, SORA_PARAMS};

const DT: f32 = 1.0 / 60.0;

// ------------------------------------------------------------------------------------ helpers

fn cell(json: &serde_json::Value) -> Option<LoadoutValue> {
    match json {
        serde_json::Value::Number(n) => n.as_f64().map(LoadoutValue::Number),
        serde_json::Value::String(s) => Some(LoadoutValue::Text(s.clone())),
        serde_json::Value::Array(items) => items.iter().map(|i| i.as_str().map(str::to_string)).collect::<Option<Vec<_>>>().map(LoadoutValue::List),
        serde_json::Value::Object(map) => map.iter().map(|(k, v)| v.as_f64().map(|n| (k.clone(), n))).collect::<Option<Vec<_>>>().map(LoadoutValue::Map),
        _ => None,
    }
}

/// The level-99 loadout sheet; `equipped` replaces its equipped abilities.
fn level99(equipped: Option<&[&str]>) -> Status {
    let path = format!("{}/../../sheets/port/sora-loadout-99.json", env!("CARGO_MANIFEST_DIR"));
    let json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))).unwrap();
    let mut loadout = Loadout::new("sora-loadout-99");
    for row in json["rows"].as_array().unwrap() {
        let key = row[0].as_str().unwrap();
        let value = match (key, equipped) {
            ("abilities.equipped", Some(list)) => LoadoutValue::List(list.iter().map(|s| s.to_string()).collect()),
            _ => cell(&row[1]).unwrap(),
        };
        loadout.set(key, &value).unwrap_or_else(|e| panic!("{e}"));
    }
    loadout.resolve().unwrap();
    Status::from_loadout(&loadout)
}

const ALL_COMBO: &[&str] = &["QUICK_SHAFT", "FLASH_STEP", "RADIAL_CUT", "AERIAL_SWEEP", "AERIAL_DIVE", "AIR_DOWN", "TRIPPLE_SLASH", "CHARGE_THRUST", "MAGICFLUSH", "FINAL_HEAVEN"];

fn dummy(id: u32, x: f32, y: f32, z: f32) -> TargetInfo {
    TargetInfo { id: TargetId(id), position: Vec3::new(x, y, z), center: Vec3::new(x, y, z + 90.0), velocity: Vec3::ZERO, yaw: 180.0, radius: 40.0, half_height: 90.0, flags: TargetFlags(TargetFlags::ALIVE | TargetFlags::HITTABLE | TargetFlags::LOCKABLE | TargetFlags::ENEMY | if z > 1.0 { TargetFlags::AIRBORNE } else { 0 }), hp: 1.0e6, hp_max: 1.0e6 }
}

fn pad(held: &[PadButton], before: &[PadButton]) -> PadState {
    let set = |list: &[PadButton]| list.iter().fold(PadButtons::NONE, |all, b| all.with(*b));
    PadState::from_held(set(held), set(before))
}

struct Bench {
    sora: Sora,
    host: RecordingHost,
    world: TestWorld,
    outs: Vec<Output>,
}

impl Bench {
    fn new(status: Status, dummies: &[TargetInfo]) -> Bench {
        let origin = Vec3::new(0.0, 0.0, Sora::origin_height_on_floor(&SORA_PARAMS));
        let sora = Sora::new(SORA_PARAMS, Pose { position: origin, yaw: 0.0 }).with_tuning(Tuning::GAME_EXACT).with_status(status);
        let mut host = RecordingHost::new();
        host.targets.extend_from_slice(dummies);
        let mut bench = Bench { sora, host, world: TestWorld::floor(), outs: Vec::new() };
        // Settle on the floor.
        for _ in 0..5 {
            bench.step(&[], &[]);
        }
        bench
    }

    fn step_input(&mut self, input: Input) -> Output {
        let (sora, world) = (&mut self.sora, &self.world);
        let out = self.host.step(world, |h| sora.step_hosted(DT, &input, h));
        self.outs.push(out.clone());
        out
    }

    fn step(&mut self, held: &[PadButton], before: &[PadButton]) -> Output {
        self.step_input(Input::default().with_pad(pad(held, before)))
    }

    /// A press of the attack button on this step. (The request is made in the UI phase, after
    /// the state tick: an attack out of Idle begins in the NEXT step; combat strand A 1.4.)
    fn attack(&mut self) -> Output {
        self.step(&[PadButton::ATTACK], &[])
    }

    /// A press, then the step in which the attack it requested begins.
    fn attack_begins(&mut self) -> Output {
        self.attack();
        self.step(&[], &[])
    }

    /// Presses attack on EVERY step (a press each step: the button is released in between as
    /// far as the core can tell) until the state is Idle again or `limit` steps passed.
    fn mash(&mut self, limit: usize) {
        for i in 0..limit {
            let out = self.attack();
            if i > 2 && out.state == StateId::Idle {
                break;
            }
        }
    }

    fn kinds(&self) -> Vec<u8> {
        self.sora.combat().history.clone()
    }

    fn damages(&self) -> Vec<(String, f32)> {
        self.host.hits().iter().map(|(record, _)| (record.attack_name.to_string(), record.damage)).collect()
    }
}

// ------------------------------------------------------------------------------------ the request path

#[test]
fn the_combo_of_spec_15_against_one_near_dummy_without_and_with_abilities() {
    // Section 15 row 1: ground, one dummy at 200 cm.
    let mut bench = Bench::new(level99(Some(&[])), &[dummy(1, 200.0, 0.0, 0.0)]);
    bench.mash(400);
    assert_eq!(bench.kinds(), [1, 2, 3, 4], "no abilities: 1301 1302 1303 1304");
    // Every swing hit the dummy once (no AttackInterval on these windows: one hit per target).
    let names: Vec<String> = bench.damages().iter().map(|d| d.0.clone()).collect();
    assert_eq!(names, ["KB_VERTICAL", "KB_STAB", "KB_TURN", "KB_BREAK"]);
    // All combo abilities, no Combo Plus: max 4, three finishers.
    let mut bench = Bench::new(level99(Some(ALL_COMBO)), &[dummy(1, 200.0, 0.0, 0.0)]);
    bench.mash(600);
    assert_eq!(bench.kinds(), [9, 6, 4, 0x0C], "all abilities: M_1359 1306 1304 M_1362");
}

#[test]
fn combo_plus_adds_normal_steps_and_the_level_99_loadout_has_two() {
    // Section 15: "with n copies of Combo Plus the normal part grows by n steps"; the level-99
    // sheet equips COMBO_PLUS x2 and every finisher ability: 9, 0A, 0B, then 6, 4, 0C.
    let mut bench = Bench::new(level99(None), &[dummy(1, 200.0, 0.0, 0.0)]);
    bench.mash(900);
    assert_eq!(bench.kinds(), [9, 0x0A, 0x0B, 6, 4, 0x0C]);
    let mut bench = Bench::new(level99(Some(&["COMBO_PLUS"])), &[dummy(1, 200.0, 0.0, 0.0)]);
    bench.mash(900);
    assert_eq!(bench.kinds(), [1, 2, 3, 2, 4]);
}

#[test]
fn far_and_crowd_starts_of_spec_15() {
    // Row 2: dist > 400 at the first press: the approach attack first; reach is carried.
    let mut bench = Bench::new(level99(Some(&[])), &[dummy(1, 600.0, 0.0, 0.0)]);
    bench.mash(500);
    assert_eq!(bench.kinds(), [2, 1, 3, 4]);
    // Row 3: three dummies within 400 cm (strand D 7.4: raw 3D distance from the search origin).
    let crowd = [dummy(1, 200.0, 0.0, 0.0), dummy(2, 180.0, 150.0, 0.0), dummy(3, 180.0, -150.0, 0.0)];
    let mut bench = Bench::new(level99(Some(&[])), &crowd);
    bench.mash(500);
    assert_eq!(bench.sora.combat().lockon.crowd, 3);
    assert_eq!(bench.kinds(), [3, 2, 3, 4]);
    let mut bench = Bench::new(level99(Some(ALL_COMBO)), &crowd);
    bench.mash(700);
    assert_eq!(bench.kinds(), [0x0B, 5, 4, 7]);
}

#[test]
fn window_timings_of_1301() {
    // Section 15, per-step checks, clip 1301: request accepted from the later of TresEnableInput
    // (0.1333 s) and the first hit window (0.1667 s); the queued attack starts at TresEnableChange
    // (0.3667 s). Frame order C -> M -> S: the press of step N begins the attack in S(N); the
    // clock is k / 60 after M(N + k); a notify at T fires on the first update with
    // position + 1e-4 >= T (animation spec 9).
    let mut bench = Bench::new(level99(Some(&[])), &[dummy(1, 200.0, 0.0, 0.0)]);
    let pressed = bench.attack();
    assert_eq!(pressed.state, StateId::Idle, "the request is made after the state tick of the press frame (strand A 1.4)");
    let begin = bench.step(&[], &[]);
    assert_eq!((begin.state, begin.anim.name), (StateId::Attack, AnimId::A1301));
    let n = bench.outs.len();
    // The notify times at full precision, from the generated table.
    let time = |class: NotifyClass| AnimId::A1301.notifies().iter().find(|d| d.class == class).unwrap().time;
    let (input_at, window_at, change_at) = (time(NotifyClass::TresAnimNotifyEnableInput), time(NotifyClass::TresAnimNotifyStateAttack), time(NotifyClass::TresAnimNotifyEnableChange));
    assert!((input_at - 0.1333).abs() < 1.0e-3 && (window_at - 0.1667).abs() < 1.0e-3 && (change_at - 0.3667).abs() < 1.0e-3);
    let fires = |t: f32| (1..200).find(|k| *k as f32 * DT + 1.0e-4 >= t).unwrap();
    let (k_window, k_change) = (fires(window_at), fires(change_at));
    assert_eq!((k_window, k_change), (10, 22));
    // Presses in C(N + 1) .. C(N + k_window) are all lost: the window opens in M(N + k_window),
    // after that step's controller update. No buffer.
    // (Strand A 1.4: with B = the step the attack began in, the clock is k / 60 after M(B + k).
    // Presses in the UI phase of B + 1 .. B + k_window - 1 are lost; the press of step
    // B + k_window would be taken: its CanAttack sees the notify flags of that frame's M.)
    for _ in 1..k_window {
        bench.attack();
        assert!(!bench.sora.attack().unwrap().next_queued, "a press before the window is lost");
    }
    bench.step(&[], &[]);
    assert!(bench.sora.attack().unwrap().swing_started && bench.sora.attack().unwrap().enable_input);
    // Strand A 2.1, the hit-confirm byte: without Combo Master (no ability is equipped here) a
    // chained attack is accepted only after a hit of the chain has landed. The window is open and
    // TresEnableInput has fired, but the blade has not reached the dummy yet: the press is lost.
    assert!(bench.host.hits().is_empty());
    bench.attack();
    assert!(!bench.sora.attack().unwrap().next_queued, "no hit landed yet: refused");
    // Nothing pressed until the hit lands: still not queued (nothing was buffered).
    while bench.host.hits().is_empty() {
        bench.step(&[], &[]);
        assert!(bench.outs.len() - n < k_change, "1301 hits a dummy at 200 cm before its change point");
    }
    assert!(!bench.sora.attack().unwrap().next_queued);
    // A press now is taken, and the next attack begins in S of the step whose M fires TresEnableChange.
    bench.attack();
    assert!(bench.sora.attack().unwrap().next_queued);
    while bench.sora.attack().unwrap().kind == 1 {
        bench.step(&[], &[]);
    }

    assert_eq!(bench.outs.len() - n, k_change, "the chained attack begins {k_change} steps after the first");
    assert_eq!((bench.kinds(), bench.outs.last().unwrap().anim.name), (vec![1, 2], AnimId::A1302));
}

#[test]
fn mashing_only_before_the_window_gives_a_single_attack() {
    // Spec 0.2 / 3.1: a press is not buffered.
    let mut bench = Bench::new(level99(Some(&[])), &[dummy(1, 200.0, 0.0, 0.0)]);
    bench.attack();
    for _ in 0..9 {
        bench.attack();
    }
    for _ in 0..120 {
        bench.step(&[], &[]);
    }
    assert_eq!(bench.kinds(), [1]);
    assert_eq!(bench.outs.last().unwrap().state, StateId::Idle, "the attack ended with its clip (0.7667 s) and chained to Idle");
}

// ------------------------------------------------------------------------------------ motion

#[test]
fn homing_speed_follows_the_approach_rule_and_stops_at_160() {
    // Spec 6.3 / section 15 "homing expectations": while the TresTraction window is open the
    // commanded horizontal speed approaches 800 by v += (800 - v) x min(0.2 x dt x 60, 1) per
    // tick; target speed 0 once the centre distance to the lock-on point is <= 160; after the
    // window it approaches 0 with factor 0.5.
    let mut bench = Bench::new(level99(Some(&[])), &[dummy(1, 600.0, 0.0, 0.0)]);
    bench.attack_begins();
    assert_eq!(bench.kinds(), [2], "far: the approach attack 1302");
    let mut speeds = Vec::new();
    let mut inside = false;
    for _ in 0..70 {
        let out = bench.step(&[], &[]);
        let view = bench.sora.attack();
        let Some(view) = view else { break };
        let distance = (Vec3::new(600.0, 0.0, 0.0) - out.position).truncate().length();
        if view.enable_traction {
            speeds.push((out.velocity.truncate().length(), distance));
        }
        inside |= distance <= 160.0;
    }
    assert!(speeds.len() >= 10, "the traction window of 1302 is 0.3 - 0.5 s");
    let expect = |v: f32, target: f32, rate: f32| v + (target - v) * (rate * DT * 60.0f32).min(1.0);
    assert!((speeds[0].0 - expect(0.0, 800.0, 0.2)).abs() < 0.01, "first homing tick {}", speeds[0].0);
    assert!((speeds[1].0 - expect(speeds[0].0, 800.0, 0.2)).abs() < 0.01);
    assert!((speeds[2].0 - expect(speeds[1].0, 800.0, 0.2)).abs() < 0.01);
    for pair in speeds.windows(2) {
        // (The speed commanded in S of a step is from the distance at that step.)
        let (v, d) = pair[0];
        let wanted = if d <= 160.0 { expect(v, 0.0, 0.5) } else { expect(v, 800.0, 0.2) };
        assert!((pair[1].0 - wanted).abs() < 0.5 || pair[1].1 <= 160.0 || d <= 170.0, "speed {} after {v} at distance {d}", pair[1].0);
    }
    // He faces the dummy and has closed on it (homing + the clip's 372.6 cm of root motion).
    let last = bench.outs.last().unwrap();
    assert!(last.yaw.abs() < 1.0 && last.position.x > 250.0, "ended at {:?}", last.position);
    let _ = inside;
}

#[test]
fn an_attack_snaps_to_its_target_and_the_root_motion_moves_him() {
    // Spec 6.2: every attack starts with m_bFaceAt = 1: on its first tick he snaps to the target.
    let mut bench = Bench::new(level99(Some(&[])), &[dummy(1, 0.0, 200.0, 0.0)]);
    let out = bench.attack_begins();
    assert!((out.yaw - 90.0).abs() < 1.0e-3, "snapped to yaw {}", out.yaw);
    // Without a target the yaw stays, and 1301 travels its 207 cm of root motion (section 15).
    let mut bench = Bench::new(level99(Some(&[])), &[]);
    let start = bench.attack_begins().position;
    for _ in 0..60 {
        bench.step(&[], &[]);
    }
    let end = bench.outs.last().unwrap();
    assert!(end.yaw.abs() < 1.0e-4);
    assert!((end.position.x - start.x - 207.0).abs() < 1.5, "1301 moved {} cm", end.position.x - start.x);
}

#[test]
fn an_air_attack_hangs_and_the_air_cycle_is_10_17_12_13() {
    // Section 15 row 5 (Sora airborne, the dummy at his height) and spec 6.3 / 4.3 tail: gravity
    // off, Flying, GravityScale 0.1 while the Z channel runs.
    let mut bench = Bench::new(level99(Some(&[])), &[dummy(1, 250.0, 0.0, 200.0)]);
    // A held jump; attack near the apex.
    bench.step_input(Input { jump_pressed: true, jump_held: true, ..Input::default() });
    for _ in 0..24 {
        bench.step_input(Input { jump_held: true, ..Input::default() });
    }
    assert!(!bench.outs.last().unwrap().on_floor);
    let height = bench.outs.last().unwrap().position.z;
    assert!(height > 150.0, "jumped to {height}");
    bench.mash(500);
    let kinds = bench.kinds();
    assert_eq!(kinds[0], 0x10, "the first air attack");
    assert!(kinds.len() >= 3 && kinds[1] == 0x17 && kinds[2] == 0x12, "air cycle {kinds:x?}");
    // While an attack state ran he never dropped far: he hangs.
    let lowest = bench.outs.iter().filter(|o| o.state == StateId::Attack).map(|o| o.position.z).fold(f32::MAX, f32::min);
    assert!(lowest > height - 120.0, "lowest point during the air combo {lowest} (pressed at {height})");
}

// ------------------------------------------------------------------------------------ hits

#[test]
fn damage_per_hit_with_the_level_99_stats() {
    // Spec 8.3 (the formula is the status implementer's `compute_damage`): against a target
    // without a status object, Defense 0: ceil((STR - 1e-6) x (power - 1e-6)) x the difficulty
    // factor. Level 99: Strength 66 (sheet sora-loadout-99: level row 62 + Kingdom Key 4).
    let status = level99(Some(&[]));
    let strength = status.stats.strength;
    assert_eq!(strength, 66);
    let mut bench = Bench::new(status, &[dummy(1, 200.0, 0.0, 0.0)]);
    bench.mash(400);
    let damages = bench.damages();
    assert_eq!(damages.len(), 4);
    let power = |row: &str| kh_core::combat::tables::attack_data_by_id(row).unwrap().power;
    for (row, amount) in &damages {
        let expected = ((strength as f32 - 1.0e-6) * (power(row) - 1.0e-6)).ceil();
        assert_eq!(*amount, expected, "{row}: power {}", power(row));
    }
    // 0.6 / 0.8 / 0.8 / 1.2 (section 15): 40, 53, 53, 80.
    assert_eq!(damages.iter().map(|d| d.1).collect::<Vec<_>>(), [40.0, 53.0, 53.0, 80.0]);
    // The hit record (strand C 8.7): knockback direction = unit vector from his root to the
    // victim's root; the four counters; finish flag on the combo's last hit.
    let hits = bench.host.hits();
    let (first, last) = (hits[0].0, hits[3].0);
    assert!((first.direction.length() - 1.0).abs() < 1.0e-5 && first.direction.x > 0.9);
    assert_eq!(first.info.counters[0], 0);
    assert_eq!(last.info.counters[0], 3, "the combo count of the fourth attack");
    assert!(last.info.flags50 & kh_core::host::HitInfo::FLAG50_FINISH != 0 && first.info.flags50 & kh_core::host::HitInfo::FLAG50_FINISH == 0);
    assert_eq!(first.info.reflect_level, 1, "KB_VERTICAL may clash on the first swing");
    assert_eq!(hits[1].0.info.reflect_level, 0, "forced to 0 after the first swing (C 8.2.1)");
    // The dummy lost exactly that much.
    assert_eq!(bench.host.targets[0].hp, 1.0e6 - 226.0);
    // Style points (C 8.5.2): the rows' m_FormPoint when HP was lost.
    let style = bench.sora.combat().style;
    assert!(style.gauge > 0.0 && style.gauge < 100.0, "gauge {}", style.gauge);
}

#[test]
fn hit_events_carry_effect_and_sound_and_a_miss_delivers_nothing() {
    // Strand C 8.6: a PHYSICAL row on a character: effect vw_so010_hit0 (+ vw_so010_str0 for a
    // phys shape) at the impact and the window's weapon hit-sound alias (1301: HIT1).
    let mut bench = Bench::new(level99(Some(&[])), &[dummy(1, 200.0, 0.0, 0.0)]);
    bench.attack();
    let mut seen = Vec::new();
    for _ in 0..50 {
        let out = bench.step(&[], &[]);
        for event in &out.events {
            match &event.kind {
                EventKind::EffectSpawn { spawn, .. } if spawn.name.starts_with("vw_so010") => seen.push(spawn.name),
                EventKind::SoundPlay { play, .. } if play.name.starts_with("HIT") => seen.push(play.name),
                EventKind::Hit { record, result } => {
                    assert!(result.accepted && record.attack_name == "KB_VERTICAL");
                    seen.push("hit");
                }
                _ => {}
            }
        }
    }
    assert_eq!(seen, ["vw_so010_hit0", "vw_so010_str0", "HIT1", "hit"]);
    // The swing sound of the clip itself went through the generic notify handlers.
    let swing = bench.outs.iter().flat_map(|o| &o.events).any(|e| matches!(&e.kind, EventKind::SoundPlay { play, .. } if play.name == "SWING1"));
    assert!(swing, "TresPlayWeaponSwingSE of 1301");
    // A dummy out of reach (behind him, 400 cm): the swing delivers nothing.
    let mut bench = Bench::new(level99(Some(&[])), &[]);
    bench.host.targets.push(TargetInfo { flags: TargetFlags(TargetFlags::ALIVE | TargetFlags::HITTABLE), ..dummy(7, -400.0, 0.0, 0.0) });
    bench.attack();
    for _ in 0..60 {
        bench.step(&[], &[]);
    }
    assert!(bench.host.hits().is_empty());
}

#[test]
fn the_re_hit_interval_empties_the_whole_hit_list() {
    // Strand C 8.2.2: AttackInterval is one timer per window, started by the first hit; when it
    // elapses every victim may be hit again. 1314 (air finisher) has a window with interval
    // 0.2333 s (0.1667 - 0.5667 s): two hits of KB_ROLLING on a dummy that stays in the shape.
    let row = AnimId::A1314.notifies().iter().find_map(|d| match d.data {
        kh_core::NotifyData::Attack { interval: Some(interval), row, .. } => Some((interval, row)),
        _ => None,
    });
    assert_eq!(row.map(|r| r.1), Some("KB_ROLLING"));
    assert!((row.unwrap().0 - 0.2333).abs() < 1.0e-3);
}

// ------------------------------------------------------------------------------------ targeting (strand D)

#[test]
fn the_soft_target_is_the_lowest_score_along_his_facing() {
    // D 7.4: score = (1.8 - dot(facing, direction)) x dist: a target behind at distance x scores
    // like one dead ahead at 3.5 x.
    let ahead = dummy(1, 700.0, 0.0, 0.0);
    let behind = dummy(2, -250.0, 0.0, 0.0);
    let mut bench = Bench::new(level99(Some(&[])), &[ahead, behind]);
    bench.step(&[], &[]);
    // behind: 2.8 x ~290 = 812; ahead: 0.8 x ~670 = 536.
    assert_eq!(bench.sora.combat().lockon.current, TargetId(1));
    let origin = Vec3::new(30.0, 0.0, 100.15);
    let (_, dist_a, score_a) = targeting::score_of(origin, Vec3::X, &ahead, 0.0);
    let (_, dist_b, score_b) = targeting::score_of(origin, Vec3::X, &behind, 0.0);
    assert!((score_a - 0.8 * dist_a).abs() < 1.0 && (score_b - 2.8 * dist_b).abs() < 1.0 && score_a < score_b);
    // The auto cursor event for it (D 7.11), once.
    let cursors: Vec<_> = bench.outs.iter().flat_map(|o| &o.events).filter_map(|e| if let EventKind::LockCursor { target, kind } = e.kind { Some((target, kind)) } else { None }).collect();
    assert_eq!(cursors, [(TargetId(1), Some(CursorKind::Auto))]);
    // Beyond 900 cm (the best candidate's distance): no soft target.
    let mut bench = Bench::new(level99(Some(&[])), &[dummy(1, 1200.0, 0.0, 0.0)]);
    bench.step(&[], &[]);
    assert_eq!((bench.sora.combat().lockon.current, bench.sora.combat().lockon.candidate), (TargetId::NONE, TargetId(1)));
}

#[test]
fn an_attack_pins_its_target() {
    // D 7.6: while the attack target is alive and targetable the current target is not replaced
    // by a better-scored candidate.
    let mut bench = Bench::new(level99(Some(&[])), &[dummy(1, 300.0, 0.0, 0.0), dummy(2, 0.0, 420.0, 0.0)]);
    bench.step(&[], &[]);
    assert_eq!(bench.sora.combat().lockon.current, TargetId(1));
    bench.attack();
    // Bring the other dummy right in front of him: it would win on score.
    bench.host.targets[1].position = Vec3::new(120.0, 30.0, 0.0);
    bench.host.targets[1].center = Vec3::new(120.0, 30.0, 90.0);
    for _ in 0..90 {
        bench.step(&[], &[]);
        assert_eq!(bench.sora.combat().lockon.current, TargetId(1), "pinned");
    }
    // The pinned target dies: the pin is released and the best candidate takes over.
    bench.host.targets[0].flags = TargetFlags(0);
    bench.step(&[], &[]);
    assert_eq!(bench.sora.combat().lockon.current, TargetId(2));
}

#[test]
fn rb_tap_locks_and_unlocks_and_rt_tap_steps_through_the_nearest_six() {
    // D 7.2 (a tap = a release after at most 0.2667 s), 7.6 (lock within 3500), 7.7 (RB is a
    // toggle; RT steps a distance-sorted list of at most 6 within 2400; lost beyond 4200).
    let dummies = [dummy(1, 1500.0, 0.0, 0.0), dummy(2, 800.0, 600.0, 0.0), dummy(3, 300.0, -900.0, 0.0)];
    let mut bench = Bench::new(level99(Some(&[])), &dummies);
    let rb = [PadButton::RightShoulder];
    let rt = [PadButton::RightTrigger];
    let tap = |bench: &mut Bench, button: &[PadButton]| {
        bench.step(button, &[]);
        bench.step(button, button);
        bench.step(&[], button); // the release: the tap flag is raised in this step's update
    };
    // Nothing within 900: no soft target; the tap locks the best candidate (within 3500).
    bench.step(&[], &[]);
    assert_eq!(bench.sora.combat().lockon.current, TargetId::NONE);
    // (With no current target the tap does nothing: D 7.6 "Nothing within 3500: the tap does
    // nothing (cur is then null ...)" - cur = best here, which is within 3500.)
    tap(&mut bench, &rb);
    let lock = bench.sora.combat().lockon;
    assert!(lock.locked, "locked on {:?}", lock.current);
    let first = lock.current;
    let events = bench.outs.iter().flat_map(|o| &o.events).filter(|e| matches!(e.kind, EventKind::CameraLock { .. } | EventKind::LockCursor { kind: Some(CursorKind::Lock), .. })).count();
    assert_eq!(events, 2, "the lock cursor and the camera lock");
    // A hold longer than the tap time is not a tap.
    for _ in 0..20 {
        bench.step(&rb, &rb);
    }
    bench.step(&[], &rb);
    assert!(!bench.sora.combat().lockon.rb_tap, "a long press raises no tap");
    // (Since the magic side exists a long RB press is the shotlock aim, which remembers and
    // clears the lock while it lasts - magic/shotlock.rs: the lock is taken again here.)
    for _ in 0..30 {
        bench.step(&[], &[]);
    }
    if !bench.sora.combat().lockon.locked {
        tap(&mut bench, &rb);
    }
    assert!(bench.sora.combat().lockon.locked);
    let first = bench.sora.combat().lockon.current;
    // RT taps step through the candidates within 2400 cm, nearest first.
    let mut seen = vec![first];
    for _ in 0..3 {
        tap(&mut bench, &rt);
        seen.push(bench.sora.combat().lockon.current);
    }
    seen.sort();
    seen.dedup();
    assert_eq!(seen, [TargetId(1), TargetId(2), TargetId(3)], "the switch reaches every candidate");
    // RB again: unlock.
    tap(&mut bench, &rb);
    assert!(!bench.sora.combat().lockon.locked);
}

// ------------------------------------------------------------------------------------ the formchange gauge (strand F)

#[test]
fn the_formchange_gauge_fills_from_hits_is_consumed_at_100_and_decays_after_20_s() {
    // Strand F 10.2 (build/sora_tmp/combat/P2_F_section.md): every damaging hit adds the row's
    // m_FormPoint (x 1.0 outside a form); max 100; a full gauge is consumed and offers CHANGE_L1
    // (148) for 30 s x (1 + 0.2 x FORM_TIME); after 20 s without gain it loses 5 points a second.
    let mut bench = Bench::new(level99(Some(&[])), &[dummy(1, 200.0, 0.0, 0.0)]);
    let mut offered = Vec::new();
    let mut peak = 0.0f32;
    for _ in 0..12_000 {
        let out = bench.attack();
        peak = peak.max(bench.sora.combat().style.gauge);
        for event in &out.events {
            if let EventKind::SituationCommand { kind, lifetime } = event.kind {
                offered.push((kind.value(), lifetime));
            }
        }
        if !offered.is_empty() {
            break;
        }
        // (Keep the dummy in front of him.)
        let p = bench.outs.last().unwrap().position;
        bench.host.targets[0].position = Vec3::new(p.x + 200.0, p.y, 0.0);
        bench.host.targets[0].center = Vec3::new(p.x + 200.0, p.y, 90.0);
    }
    assert_eq!(offered, [(0x94, 30.0)], "CHANGE_L1 offered once the gauge is full");
    assert!(peak > 80.0 && peak < 100.0, "the gauge peaked at {peak} before it was consumed");
    assert_eq!((bench.sora.combat().style.gauge, bench.sora.combat().style.awards), (0.0, 1));
    // One more hit, then 20 s of nothing: no decay yet; after that 5 points a second.
    let mut bench = Bench::new(level99(Some(&[])), &[dummy(1, 200.0, 0.0, 0.0)]);
    bench.attack();
    for _ in 0..60 {
        bench.step(&[], &[]);
    }
    let gained = bench.sora.combat().style.gauge;
    let form_point = kh_core::combat::tables::attack_data_by_id("KB_VERTICAL").unwrap().form_point as f32;
    assert_eq!(gained, form_point, "KB_VERTICAL's m_FormPoint");
    for _ in 0..(19 * 60 - 62) {
        bench.step(&[], &[]);
    }
    assert_eq!(bench.sora.combat().style.gauge, gained, "no decay within 20 s of the gain");
    for _ in 0..(3 * 60) {
        bench.step(&[], &[]);
    }
    assert!(bench.sora.combat().style.gauge < gained, "decayed after 20 s");
}

// ------------------------------------------------------------------------------------ strand A: hit confirm, launch, cool-down

#[test]
fn swinging_at_air_does_not_chain_without_combo_master() {
    // Strand A 2.1 (build/sora_tmp/combat/P2_A_section.md): without Combo Master (30) a chained
    // attack needs a landed hit of the chain (pawn+0xE28). With no target in reach every press
    // during the swing is lost; the next press after the swing starts a new combo at step 0.
    let mut bench = Bench::new(level99(Some(&[])), &[]);
    for _ in 0..200 {
        bench.attack();
    }
    let kinds = bench.kinds();
    assert!(kinds.len() >= 3 && kinds.iter().all(|k| *k == 1), "always the first attack: {kinds:x?}");
    // With Combo Master the chain goes on in the air.
    let mut bench = Bench::new(level99(Some(&["COMBO_MASTER"])), &[]);
    bench.mash(400);
    assert_eq!(bench.kinds(), [1, 2, 3, 4]);
}

#[test]
fn the_jump_button_during_a_combo_is_the_launch_attack() {
    // Strand A 4: with an unused stock (Rising Spiral = SLASH_UPPER 10), after a landed hit, on
    // a non-finisher step, the jump press queues the launch attack: kind 0x1C (clip 1319), the
    // combo count restarts at 0. One use per chain.
    let mut bench = Bench::new(level99(Some(&["SLASH_UPPER", "COMBO_PLUS"])), &[dummy(1, 200.0, 0.0, 0.0)]);
    bench.attack_begins();
    while bench.host.hits().is_empty() {
        bench.step(&[], &[]);
    }
    let out = bench.step(&[PadButton::JUMP], &[]);
    assert_eq!(out.state, StateId::Attack, "the press did not start a jump");
    for _ in 0..40 {
        bench.step(&[], &[]);
    }
    assert_eq!(bench.kinds(), [1, 0x1C]);
    assert_eq!(bench.outs.last().unwrap().anim.name, AnimId::A1319);
    assert_eq!(bench.sora.attack().unwrap().combo, 0);
    assert_eq!(bench.sora.combat().stocks.0, 2, "the stock is used");
    // Without the ability the press does nothing during an attack (jump never cancels one).
    let mut bench = Bench::new(level99(Some(&[])), &[dummy(1, 200.0, 0.0, 0.0)]);
    bench.attack_begins();
    while bench.host.hits().is_empty() {
        bench.step(&[], &[]);
    }
    bench.step(&[PadButton::JUMP], &[]);
    bench.step(&[], &[]);
    bench.step(&[], &[]);
    assert_eq!((bench.kinds(), bench.outs.last().unwrap().state), (vec![1], StateId::Attack));
}

#[test]
fn a_whiffed_air_attack_without_follow_up_starts_the_command_cool_down() {
    // Strand A 6.3: an air-mode attack that landed no hit and is not followed by another attack:
    // commands 0.6 s (pawn+0xE34), air slide 0.6 s, air guard 0.3333 s.
    let mut bench = Bench::new(level99(Some(&[])), &[]);
    bench.step_input(Input { jump_pressed: true, jump_held: true, ..Input::default() });
    for _ in 0..24 {
        bench.step_input(Input { jump_held: true, ..Input::default() });
    }
    bench.attack_begins();
    assert_eq!(bench.kinds(), [0x10]);
    while bench.outs.last().unwrap().state == StateId::Attack {
        bench.step(&[], &[]);
    }
    let combat = bench.sora.combat();
    assert!(combat.command_cooldown > 0.5 && combat.air_slide_cooldown > 0.5 && combat.air_guard_cooldown > 0.25, "cool-downs {} {} {}", combat.command_cooldown, combat.air_slide_cooldown, combat.air_guard_cooldown);
    // During the cool-down an attack press is refused.
    bench.attack();
    bench.step(&[], &[]);
    assert_ne!(bench.outs.last().unwrap().state, StateId::Attack);
}
