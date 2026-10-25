//! Combat, fourth pass (2026-10-12): what the P3 strand items G 1 - 4 / H 3 - 5 settled
//! (docs/sora-combat-spec.md 16.9), the attack camera (7.12.1), the HOPPING / SLOPESLIDE attack
//! requests, the shot location, the sneeze latch, and the audit's leftover tests (OVER_TIME,
//! ENDLESS_MAGIC's combo-step half, SUMMERSALT's gate, Sonic Cleave's state bits). One test per
//! feature added.

use kh_core::combat::badstat::kind as bad;
use kh_core::enums::ETresAbilityKind as Ability;
use kh_core::enums::ETresDamageKind as Kind;
use kh_core::host::EventKind;
use kh_core::states::HitOnPlayer;
use kh_core::status::LoadoutValue;
use kh_core::test_world::TestWorld;
use kh_core::{Input, Loadout, Output, PadButton, PadButtons, PadState, Pose, RecordingHost, Sora, StateId, Status, TargetFlags, TargetId, TargetInfo, Tuning, Vec2, Vec3, SORA_PARAMS};

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

/// The level-99 loadout sheet; `equipped` replaces its equipped abilities, `without` removes
/// names from the sheet's own list.
fn loadout99(equipped: Option<&[&str]>, without: &[&str]) -> Status {
    let path = format!("{}/../../sheets/port/sora-loadout-99.json", env!("CARGO_MANIFEST_DIR"));
    let json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))).unwrap();
    let mut loadout = Loadout::new("sora-loadout-99");
    for row in json["rows"].as_array().unwrap() {
        let key = row[0].as_str().unwrap();
        let value = match (key, equipped) {
            ("abilities.equipped", Some(list)) => LoadoutValue::List(list.iter().map(|s| s.to_string()).collect()),
            ("abilities.equipped", None) => match cell(&row[1]).unwrap() {
                LoadoutValue::List(list) => LoadoutValue::List(list.into_iter().filter(|n| !without.contains(&n.as_str())).collect()),
                other => other,
            },
            _ => cell(&row[1]).unwrap(),
        };
        loadout.set(key, &value).unwrap_or_else(|e| panic!("{e}"));
    }
    loadout.resolve().unwrap();
    Status::from_loadout(&loadout)
}

fn level99(equipped: Option<&[&str]>) -> Status {
    loadout99(equipped, &[])
}

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
    fn at(status: Status, dummies: &[TargetInfo], world: TestWorld, origin: Vec3) -> Bench {
        let sora = Sora::new(SORA_PARAMS, Pose { position: origin, yaw: 0.0 }).with_tuning(Tuning::GAME_EXACT).with_status(status);
        let mut host = RecordingHost::new();
        host.targets.extend_from_slice(dummies);
        Bench { sora, host, world, outs: Vec::new() }
    }

    fn in_world(status: Status, dummies: &[TargetInfo], world: TestWorld) -> Bench {
        let mut bench = Bench::at(status, dummies, world, Vec3::new(0.0, 0.0, Sora::origin_height_on_floor(&SORA_PARAMS)));
        for _ in 0..5 {
            bench.step(&[], &[]);
        }
        bench
    }

    fn new(status: Status, dummies: &[TargetInfo]) -> Bench {
        Bench::in_world(status, dummies, TestWorld::floor())
    }

    fn step_with(&mut self, held: &[PadButton], before: &[PadButton], stick: Vec2) -> Output {
        let input = Input { move_stick: stick, ..Input::default() }.with_pad(pad(held, before));
        let (sora, world) = (&mut self.sora, &self.world);
        let out = self.host.step(world, |h| sora.step_hosted(DT, &input, h));
        self.outs.push(out.clone());
        out
    }

    fn step(&mut self, held: &[PadButton], before: &[PadButton]) -> Output {
        self.step_with(held, before, Vec2::ZERO)
    }

    fn tap(&mut self, button: PadButton) -> Output {
        self.step(&[button], &[])
    }

    fn idle(&mut self, steps: usize) -> Output {
        for _ in 1..steps {
            self.step(&[], &[]);
        }
        self.step(&[], &[])
    }

    /// Steps without input until `test` holds; the number of steps taken (panics after `limit`).
    fn until(&mut self, limit: usize, what: &str, test: impl Fn(&Bench, &Output) -> bool) -> usize {
        for n in 1..=limit {
            let out = self.step(&[], &[]);
            if test(self, &out) {
                return n;
            }
        }
        panic!("{what}: not within {limit} steps; states {:?}, clips {:?}", self.states(), self.clips(0));
    }

    fn events_from(&self, from: usize) -> impl Iterator<Item = &EventKind> {
        self.outs[from..].iter().flat_map(|o| o.events.iter().map(|e| &e.kind))
    }

    fn states(&self) -> Vec<StateId> {
        let mut out: Vec<StateId> = Vec::new();
        for o in &self.outs {
            if out.last() != Some(&o.state) {
                out.push(o.state);
            }
        }
        out
    }

    fn clips(&self, from: usize) -> Vec<&'static str> {
        let mut out: Vec<&'static str> = Vec::new();
        for o in &self.outs[from..] {
            if out.last() != Some(&o.anim.name.name()) {
                out.push(o.anim.name.name());
            }
        }
        out
    }

    /// The attack-camera requests and camera locks sent since step `from`, in order: (mode,
    /// restore) for `AttackCamera`, (100 + target id, 0) for `CameraLock` (100 = unlock... id 0).
    fn camera_events(&self, from: usize) -> Vec<(i32, f32)> {
        self.events_from(from)
            .filter_map(|e| match e {
                EventKind::AttackCamera { mode, restore } => Some((*mode as i32, *restore)),
                EventKind::CameraLock { target } => Some((if *target == TargetId::NONE { 100 } else { 100 + target.0 as i32 }, 0.0)),
                _ => None,
            })
            .collect()
    }
}

/// A hit on Sora from 2 m in front of him, carrying a bad status.
fn status_hit(kind: Kind, amount: f32, status: &'static str, time: f32) -> HitOnPlayer {
    HitOnPlayer { direction: Vec3::new(-1.0, 0.0, 0.0), causer_yaw: 180.0, bad_status: status, bad_time: time, ..HitOnPlayer::new(kind, amount, TargetId(1), Vec3::new(200.0, 0.0, 0.0)) }
}

/// Fills the gauge, takes the formchange and waits out the transformation; `level` further full
/// gauges raise the finish command (1 Stun Impact, 2 Sonic Rave, 3 Last Arcanum).
fn second_form(bench: &mut Bench, level: u32) {
    bench.sora.debug_add_style(100.0);
    bench.idle(15);
    bench.tap(PadButton::SITUATION);
    assert_eq!(bench.step(&[], &[]).state, StateId::Stylechange);
    bench.idle(200);
    for _ in 1..level {
        bench.sora.debug_add_style(100.0);
        bench.step(&[], &[]);
    }
    bench.idle(20);
}

/// Uses the finish command and every extra command on show (at most `presses`), to the end of
/// STYLEFINISH. Returns the step index it started at and the highest count seen.
fn finish_command(bench: &mut Bench, presses: usize) -> (usize, i32) {
    let from = bench.outs.len();
    bench.tap(PadButton::SITUATION);
    assert_eq!(bench.step(&[], &[]).state, StateId::Stylefinish);
    let (mut used, mut max_count) = (0, 0);
    for _ in 0..1500 {
        let press = bench.sora.combat().situation.extra.is_some() && used < presses;
        let before = bench.sora.finish().map_or(0, |f| f.pending);
        let out = if press { bench.tap(PadButton::SITUATION) } else { bench.step(&[], &[]) };
        if let Some(f) = bench.sora.finish() {
            max_count = max_count.max(f.count);
            if before == 0 && f.pending != 0 {
                used += 1;
            }
        }
        if out.state != StateId::Stylefinish {
            break;
        }
    }
    (from, max_count)
}

/// Jumps and, near the top, presses the attack button with the forced kind `kind` on offer.
fn forced_in_the_air(bench: &mut Bench, kind: u8) -> usize {
    for n in 0..200 {
        let out = bench.step(&[PadButton::JUMP], if n == 0 { &[] } else { &[PadButton::JUMP] });
        if n > 10 && out.velocity.z <= 0.0 && !out.on_floor {
            break;
        }
    }
    bench.sora.combat_mut().guard.offer = Some(kh_core::combat::guard::CounterOffer { source: StateId::Jump, kind, target: TargetId(1), just_guard: false, push: false });
    bench.tap(PadButton::ATTACK);
    bench.until(10, "the attack state", |_, o| o.state == StateId::Attack);
    bench.outs.len() - 1
}

// ------------------------------------------------------------------------------------ 7.12.1 the attack camera

/// Spec 7.12.1 (controller 0x1414122A0): the burn asks for mode 4 and switches it off when its end
/// clip starts; Buzz Saw (kind 0x38) asks for mode 5 and its End switches it off; Sonic Rave puts
/// the lock-on camera on its target for the dashes (mode 3) and, without a manual lock, unlocks
/// when the dashes end. An "off" without a mode sends nothing.
#[test]
fn the_attack_camera_of_the_burn_buzz_saw_and_sonic_rave() {
    // The burn.
    let mut bench = Bench::new(level99(None), &[dummy(1, 200.0, 0.0, 0.0)]);
    bench.sora.take_hit(status_hit(Kind::Small, 10.0, "BURN", 1.5));
    bench.until(120, "the burn state", |_, o| o.state == StateId::BadstatBurn);
    assert_eq!(bench.sora.combat().attack_cam.mode, 4);
    assert_eq!(bench.camera_events(0), [(4, -1.0)], "one request: the off before it had no mode to end");
    bench.until(900, "the end of the burn", |_, o| o.state != StateId::BadstatBurn);
    assert_eq!(bench.camera_events(0), [(4, -1.0), (0, -1.0)]);
    assert_eq!(bench.sora.combat().attack_cam.mode, 0);
    let off_at = bench.outs.iter().position(|o| o.events.iter().any(|e| matches!(e.kind, EventKind::AttackCamera { mode: 0, .. }))).unwrap();
    assert_eq!(bench.outs[off_at].anim.name.name(), "BadStatBurnEnd", "off in the tick that starts the end clip (sub-step 3)");

    // Buzz Saw: mode 5 for the attack.
    let mut bench = Bench::new(level99(None), &[dummy(1, 300.0, 0.0, 250.0)]);
    let from = forced_in_the_air(&mut bench, 0x38);
    assert_eq!(bench.sora.attack().unwrap().kind, 0x38);
    assert_eq!((bench.camera_events(from), bench.sora.combat().attack_cam.mode), (vec![(5, -1.0)], 5));
    bench.until(400, "the end of the attack", |_, o| o.state != StateId::Attack);
    assert_eq!(bench.camera_events(from), [(5, -1.0), (0, -1.0)]);

    // Sonic Rave: the camera locks onto the target at the first dash and lets go at the end.
    let mut bench = Bench::new(level99(Some(&[])), &[dummy(1, 300.0, 0.0, 0.0)]);
    second_form(&mut bench, 2);
    let (from, _) = finish_command(&mut bench, 99);
    let cam = bench.camera_events(from);
    assert_eq!(cam, [(101, 0.0), (0, -1.0), (100, 0.0)], "lock on dummy 1, then off: unlock (no manual lock)");
    let lock_at = from + bench.outs[from..].iter().position(|o| o.events.iter().any(|e| matches!(e.kind, EventKind::CameraLock { .. }))).unwrap();
    assert_eq!(bench.outs[lock_at].anim.name.name(), "M_1425", "with the first dash");
    assert_eq!(bench.sora.combat().attack_cam.mode, 0);

    // ... and with a manual lock (RB tap) the off re-locks the camera on it.
    let mut bench = Bench::new(level99(Some(&[])), &[dummy(1, 300.0, 0.0, 0.0)]);
    second_form(&mut bench, 2);
    bench.tap(PadButton::RightShoulder);
    bench.idle(10);
    assert!(bench.sora.combat().lockon.locked);
    let (from, _) = finish_command(&mut bench, 0);
    assert_eq!(bench.camera_events(from), [(101, 0.0), (0, -1.0), (101, 0.0)]);
}

// ------------------------------------------------------------------------------------ 10.9 the finish commands' loose ends

/// Spec 10.9 items 1, 2 and 4 as settled: Last Arcanum's count rises with every hit window that
/// opens, in every sub-step (the class's own `TresAtk` begin handler 0x1413DD7D0) - seven in
/// `M_1427`, more with the break clips; `finishEnd` is never set, so none of its hits carries the
/// finish flag; Stun Impact's first hit window sends the rumble effect 1.
#[test]
fn last_arcanum_counts_every_window_and_stun_impact_rumbles() {
    let mut bench = Bench::new(level99(Some(&[])), &[dummy(1, 300.0, 0.0, 0.0)]);
    second_form(&mut bench, 3);
    let (from, count) = finish_command(&mut bench, 0);
    assert_eq!(bench.clips(from).iter().filter(|c| c.starts_with("M_14")).count(), 1, "no extra used: M_1427 alone");
    assert_eq!(count, 7, "the seven hit windows of M_1427");
    let mut bench = Bench::new(level99(Some(&[])), &[dummy(1, 300.0, 0.0, 0.0)]);
    second_form(&mut bench, 3);
    let (_, count) = finish_command(&mut bench, 99);
    assert!(count > 7, "the break clips' windows count on: {count}");
    let arcanum: Vec<_> = bench.host.hits().into_iter().filter(|(r, _)| r.attack_name.starts_with("LM_LASTARCANUM")).collect();
    assert!(arcanum.len() >= 8, "{}", arcanum.len());
    // info+0x50 bit 0 = `finish` (activation S'+0x5C bit 0 = state.finishEnd).
    assert!(arcanum.iter().all(|(r, _)| r.info.flags50 & 1 == 0), "finishEnd stays 0 through Last Arcanum");

    let mut bench = Bench::new(level99(Some(&[])), &[dummy(1, 300.0, 0.0, 0.0)]);
    second_form(&mut bench, 1);
    let (from, _) = finish_command(&mut bench, 0);
    let rumbles: Vec<_> = bench.outs[from..].iter().enumerate().filter(|(_, o)| o.events.iter().any(|e| matches!(e.kind, EventKind::ForceFeedback { effect: 1, looping: false }))).map(|(i, _)| i).collect();
    assert_eq!(rumbles.len(), 1, "one rumble");
    let slow_off = bench.outs[from..].iter().position(|o| o.events.iter().any(|e| matches!(e.kind, EventKind::TimeDilation { rate, .. } if rate == 1.0))).unwrap();
    assert_eq!(rumbles[0], slow_off, "in the tick the world slow ends (the first hit window)");
}

// ------------------------------------------------------------------------------------ 4.3 / 6.3 the kinds' state bits

/// PlayAttack's per-kind bits state+0x234 bit 3 and state+0x23C bit 4 (kinds 0x1C Rising Spiral
/// and 0x37 Sonic Cleave): while the traction window is open the Z channel writes 0 - he keeps
/// his height but for the clip's root motion - and DirZ is not recomputed. Sonic Cleave comes
/// down 500 cm with its clip (1182) and gets its -1000 only when the window closes at 1.0 s; a
/// cleave that hits nothing spawns no shockwave.
#[test]
fn rising_spiral_and_sonic_cleave_write_no_height_change_in_the_traction_window() {
    // Sonic Cleave from high above a dummy that is out of reach of the blade (a whiff).
    let mut bench = Bench::new(loadout99(Some(&["HIGHJUMP", "SONIC_DOWN"]), &[]), &[dummy(1, 160.0, 0.0, -2000.0)]);
    let from = forced_in_the_air(&mut bench, 0x37);
    assert_eq!(bench.sora.attack().unwrap().kind, 0x37);
    let z0 = bench.outs[from].position.z;
    let mut window = Vec::new();
    for _ in 0..200 {
        let out = bench.step(&[], &[]);
        if out.state != StateId::Attack {
            break;
        }
        if out.anim.name.name() == "1182" && out.anim.time > 0.05 && out.anim.time < 0.95 {
            window.push(out.velocity.z);
        }
    }
    assert!(window.len() > 40 && window.iter().all(|vz| *vz == 0.0), "vz inside the window: {:?}", &window[..window.len().min(6)]);
    let at_one = bench.outs[from..].iter().find(|o| o.anim.name.name() == "1182" && o.anim.time >= 1.0).expect("the clip reaches 1.0 s");
    assert!(z0 - at_one.position.z > 350.0 && z0 - at_one.position.z < 520.0, "down by the root motion: {}", z0 - at_one.position.z);
    let after = bench.outs[from..].iter().filter(|o| o.anim.name.name() == "1182" && o.anim.time > 1.02 && o.anim.time < 1.1).map(|o| o.velocity.z).fold(0.0f32, f32::min);
    assert!(after < -300.0, "DirZ (-1000, kept by state+0x23C bit 4) is written once the window has closed: {after}");
    assert_eq!(bench.sora.combat().shots_spawned, 0, "no hit, no landing inside the window: no SHOCKWAVE_S");

    // Rising Spiral: the jump press during a ground combo. No height change by velocity inside its
    // window 0.1 - 0.6 s.
    let mut bench = Bench::new(level99(Some(&["COMBO_MASTER", "SLASH_UPPER"])), &[dummy(1, 150.0, 0.0, 0.0)]);
    bench.tap(PadButton::ATTACK);
    bench.until(10, "the attack state", |_, o| o.state == StateId::Attack);
    let mut launched = None;
    for _ in 0..120 {
        let out = bench.tap(PadButton::JUMP);
        bench.step(&[], &[]);
        if bench.sora.attack().is_some_and(|a| a.kind == 0x1C) {
            launched = Some(bench.outs.len());
            break;
        }
        assert_eq!(out.state, StateId::Attack);
    }
    let from = launched.expect("the launch attack");
    let mut window = Vec::new();
    for _ in 0..80 {
        let out = bench.step(&[], &[]);
        if out.anim.name.name() == "1319" && out.anim.time > 0.15 && out.anim.time < 0.55 {
            window.push(out.velocity.z);
        }
    }
    assert!(window.len() > 15 && window.iter().all(|vz| *vz == 0.0), "{:?} (from {from})", &window[..window.len().min(6)]);
}

// ------------------------------------------------------------------------------------ 3.3 attacks out of HOPPING and SLOPESLIDE

/// TryAttack cases 9 / 10 and the head of PlayAttack: an attack press on a hop point is the
/// ordinary air attack 0x10 (clip 1311) whatever the reach; out of a slope slide it is the
/// slope-slide attack 0x2F (clip `M_1143`, row RUN_SLOPE) - or 0x10 when the target is more than
/// 30 cm above him.
#[test]
fn attacks_out_of_a_slope_slide_and_off_a_hop_point() {
    use kh_core::world::{surface, HopPoint};
    let slope = || {
        let n = Vec3::new(30.0f32.to_radians().sin(), 0.0, 30.0f32.to_radians().cos());
        TestWorld::floor().with_plane_through(Vec3::new(0.0, 0.0, 1500.0), n).with_flags(2, surface::WALL_KICK | surface::SLOPE_SLIDE).with_flags(1, surface::WALL_KICK | surface::SLOPE_SLIDE)
    };
    let slide = |target: TargetInfo| {
        let mut bench = Bench::at(level99(None), &[target], slope(), Vec3::new(0.0, 0.0, 1560.0));
        bench.until(60, "the slope slide", |_, o| o.state == StateId::Slopeslide);
        bench.idle(10);
        bench.tap(PadButton::ATTACK);
        bench.until(6, "the attack state", |_, o| o.state == StateId::Attack);
        let attack = bench.sora.attack().unwrap();
        (attack.kind, attack.air, bench.outs.last().unwrap().anim.name.name(), bench)
    };
    // A dummy down the slope, below him.
    let (kind, air, clip, mut bench) = slide(dummy(1, 900.0, 0.0, 900.0));
    assert_eq!((kind, air, clip), (0x2F, true, "M_1143"));
    bench.until(200, "the end of the attack", |_, o| o.state != StateId::Attack);
    // The row's power 1.5 TOSS, when it lands.
    for (record, _) in bench.host.hits() {
        assert_eq!((record.attack_name, record.damage_kind), ("RUN_SLOPE", Kind::Toss));
    }
    // A dummy above him: the ordinary air attack.
    let (kind, air, clip, _) = slide(dummy(1, 300.0, 0.0, 1700.0));
    assert_eq!((kind, air, clip), (0x10, true, "1311"));

    // Off a hop point.
    let z = Sora::origin_height_on_floor(&SORA_PARAMS) + 100.0;
    let mut world = TestWorld::floor();
    world.hops.push(HopPoint { id: 1, position: Vec3::new(500.0, 0.0, z), check_radius: 100.0, idle_motion: 1, end_point: false, end_roll: false, active: true });
    let mut bench = Bench::at(level99(None), &[dummy(1, 900.0, 0.0, 0.0)], world, Vec3::new(330.0, 0.0, Sora::origin_height_on_floor(&SORA_PARAMS)));
    bench.tap(PadButton::GUARD);
    bench.until(70, "the hop state", |_, o| o.state == StateId::Hopping);
    bench.until(60, "balancing on the point", |_, o| o.anim.name.name().starts_with("Balance"));
    bench.tap(PadButton::ATTACK);
    bench.until(6, "the attack state", |_, o| o.state == StateId::Attack);
    let attack = bench.sora.attack().unwrap();
    assert_eq!((attack.kind, attack.air, bench.outs.last().unwrap().anim.name.name()), (0x10, true, "1311"));
}

// ------------------------------------------------------------------------------------ the audit's leftovers

/// OVER_TIME (172; spec 11.3): a situation command that is offered again while it is in the
/// list keeps its longer time with the ability (its countdown starts over at that time);
/// without it the new, shorter time replaces the old one.
#[test]
fn overtime_keeps_the_longer_time_of_a_command_offered_again() {
    let run = |over_time: bool| {
        // FORM_TIME x1: the formchange command lives 36 s.
        let mut status = level99(Some(&["FORM_TIME"]));
        if over_time {
            status.abilities.add_worn(Ability::OverTime);
        }
        let mut bench = Bench::new(status, &[]);
        bench.sora.debug_add_style(100.0);
        bench.idle(600);
        let first = bench.sora.combat().situation.list[0];
        assert!((first.total - 36.0).abs() < 1.0e-3 && first.remaining < 27.0, "{first:?}");
        // The ability is taken off: the same command offered again comes with 30 s.
        bench.sora.status_mut().abilities.set(Ability::FormTime, 1, 0);
        bench.sora.debug_add_style(100.0);
        bench.step(&[], &[]);
        let again = bench.sora.combat().situation.list[0];
        assert_eq!(bench.sora.combat().situation.list.len(), 1, "never two entries of one kind");
        (again.total, again.remaining)
    };
    let (total, remaining) = run(false);
    assert!((total - 30.0).abs() < 1.0e-3 && remaining > 29.9, "without OVER_TIME the new time replaces it: {total} {remaining}");
    let (total, remaining) = run(true);
    assert!((total - 36.0).abs() < 1.0e-3 && remaining > 35.9, "with OVER_TIME the longer time stays and starts over: {total} {remaining}");
}

/// ENDLESS_MAGIC's combat half (spec 5.5: slot 115 with pawn vt+0x1D20): a hit window of a MAGIC
/// row (Magic Flash's `KB_FLASH`) does not advance the combo step state+0x12C while the ability
/// is on him; a physical row still does.
#[test]
fn endless_magic_freezes_the_combo_step_for_magic_rows() {
    let run = |endless: bool| {
        let mut status = level99(Some(&["COMBO_MASTER", "MAGICFLUSH"]));
        if endless {
            status.abilities.add_worn(Ability::EndlessMagic);
        }
        let mut bench = Bench::new(status, &[dummy(1, 150.0, 0.0, 0.0)]);
        // (attack kind, the highest state+0x12C seen during it)
        let mut steps: Vec<(u8, i32)> = Vec::new();
        for _ in 0..600 {
            let out = bench.tap(PadButton::ATTACK);
            bench.step(&[], &[]);
            let ctx = bench.sora.combat().attack_ctx;
            if ctx.state {
                match steps.last_mut() {
                    Some(last) if last.0 == ctx.kind => last.1 = last.1.max(ctx.counters[1]),
                    _ => steps.push((ctx.kind, ctx.counters[1])),
                }
            }
            if steps.len() >= 4 && out.state == StateId::Idle {
                break;
            }
        }
        steps
    };
    let plain = run(false);
    // (MaxCombo 4 with two finishers: the plain one, kind 4, then Magic Flash, kind 7.)
    assert_eq!(plain.iter().map(|s| s.0).collect::<Vec<_>>()[..4], [1, 2, 4, 7], "the last finisher is Magic Flash: {plain:?}");
    assert_eq!((plain[2].1, plain[3].1), (3, 4), "one per hit window: {plain:?}");
    let endless = run(true);
    assert_eq!((endless[2].1, endless[3].1), (3, 3), "the magic row's window does not count: {endless:?}");
}

/// SUMMERSALT's gate (Attack slot 124 0x1413325F0): the attack out of a bar swing is forced as
/// 0x39; without the ability it becomes the ordinary air attack of the reach (0x10, upgraded to
/// 0x18 by QUICK_SHAFT), with a target again.
#[test]
fn without_somersault_the_bar_swing_attack_is_an_ordinary_air_attack() {
    use kh_core::world::Pole;
    let swing = |status: Status| {
        let mut world = TestWorld::floor();
        world.poles.push(Pole { id: 7, base1: Vec3::new(900.0, -200.0, 500.0), base2: Vec3::new(900.0, 200.0, 500.0), check_radius: 50.0, attach_radius: 30.0, horizontal: true, active: true });
        let mut bench = Bench::in_world(status, &[dummy(1, 1300.0, 0.0, 0.0)], world);
        bench.step(&[PadButton::JUMP], &[]);
        for _ in 0..120 {
            let out = bench.step(&[PadButton::JUMP], &[PadButton::JUMP]);
            if out.state == StateId::Jump && out.sub_step == 4 {
                break;
            }
        }
        bench.step_with(&[PadButton::GUARD], &[], Vec2::new(0.0, 1.0));
        bench.until(60, "the bar", |_, o| o.state == StateId::PoleTurn);
        bench.idle(20);
        bench.tap(PadButton::ATTACK);
        bench.until(6, "the attack state", |_, o| o.state == StateId::Attack);
        let attack = bench.sora.attack().unwrap();
        (attack.kind, bench.outs.last().unwrap().anim.name.name())
    };
    assert_eq!(swing(level99(None)), (0x39, "1184"));
    assert_eq!(swing(loadout99(None, &["SUMMERSALT"])), (0x18, "M_1363"), "QUICK_SHAFT upgrades the fallback 0x10");
    assert_eq!(swing(loadout99(None, &["SUMMERSALT", "QUICK_SHAFT"])), (0x10, "1311"));
}

// ------------------------------------------------------------------------------------ 8.8 / 8.2.2 / 7.10 / 9.9.5

/// Spec 8.8: a shot starts at the weapon's `MuzzleAttachPoint` socket = `ATK_01`, the keyblade's
/// tip: the finisher air-down's shockwave appears where the tip is when it lands - in front of
/// him and near the floor - not at the hilt.
#[test]
fn a_shot_starts_at_the_keyblades_tip() {
    let mut bench = Bench::new(level99(Some(&["HIGHJUMP", "AIR_DOWN"])), &[dummy(1, 160.0, 0.0, 0.0)]);
    forced_in_the_air(&mut bench, 0x0E);
    let mut shot = None;
    for _ in 0..400 {
        let out = bench.step(&[], &[]);
        if let Some(s) = bench.sora.combat().shots.first() {
            shot = Some((s.position, out.position, out.yaw));
            break;
        }
    }
    let (at, sora, yaw) = shot.expect("DOWNIMPACT");
    let forward = Vec3::new(yaw.to_radians().cos(), yaw.to_radians().sin(), 0.0);
    let ahead = (at - sora).dot(forward);
    // (Clip 1318 at the landing: ATK_00 the hilt is at his hand, ATK_01 the tip about a blade's
    // length further out and lower.)
    assert!(ahead > 60.0, "the tip is well in front of him: {ahead} ({at} vs {sora})");
    assert!(at.z - sora.z < 60.0, "and low: {}", at.z - sora.z);
}

/// Spec 8.2.2 (P3 H3): the attack-collision manager ticks after the pawn's whole tick, in the
/// same frame: a target that stands inside the blade when the window opens is hit in the step
/// whose pose tick opened it, not one step later. (A ring of dummies all round him: wherever the
/// blade is at the opening pose of `1301`, one of them is in it.)
#[test]
fn a_window_sweeps_in_the_frame_that_opens_it() {
    let ring: Vec<TargetInfo> = (0..12).map(|i| {
        let a = (i as f32 * 30.0).to_radians();
        // (Tall and wide: the blade starts the swing above his head.)
        TargetInfo { center: Vec3::new(110.0 * a.cos(), 110.0 * a.sin(), 250.0), radius: 60.0, half_height: 250.0, ..dummy(i + 1, 110.0 * a.cos(), 110.0 * a.sin(), 0.0) }
    }).collect();
    let mut bench = Bench::new(level99(None), &ring);
    bench.tap(PadButton::ATTACK);
    let mut opened = false;
    for _ in 0..60 {
        let out = bench.step(&[], &[]);
        let hit = out.events.iter().any(|e| matches!(e.kind, EventKind::Hit { .. }));
        if bench.sora.attack().is_some_and(|a| a.swing_started) {
            opened = true;
            assert!(hit, "hit in the opening frame (clip time {})", out.anim.time);
            assert!(bench.sora.combat().swing_hit);
            break;
        }
        assert!(!hit, "no hit before the window opens");
    }
    assert!(opened);
}

/// Spec 7.10 (P3 H4): while his push body touches the body of the attack's target and that
/// target has `m_bRushSuppression`, the swing stops homing (speed target 0) and its root-scale
/// channel writes 0.1; a target without the flag changes nothing.
#[test]
fn rush_suppression_stops_the_advance_while_the_bodies_touch() {
    let advance = |suppress: bool| {
        // 60 cm from a dummy with a push body (level 9: he does not push it): the bodies overlap
        // from the first frame of the swing; 1304-class root-scale kinds would carry him through.
        let mut bench = Bench::new(level99(Some(&[])), &[dummy(1, 85.0, 0.0, 0.0)]);
        bench.host.push_levels.push((TargetId(1), 4));
        if suppress {
            bench.host.rush_suppression.push(TargetId(1));
        }
        bench.sora.combat_mut().guard.offer = Some(kh_core::combat::guard::CounterOffer { source: StateId::Idle, kind: 0x21, target: TargetId(1), just_guard: false, push: false });
        bench.tap(PadButton::ATTACK);
        bench.until(10, "the attack state", |_, o| o.state == StateId::Attack);
        let (x0, mut furthest, mut rush) = (bench.outs.last().unwrap().position.x, f32::MIN, false);
        for _ in 0..40 {
            let out = bench.step(&[], &[]);
            furthest = furthest.max(out.position.x - x0);
            rush |= bench.sora.attack().is_some_and(|a| a.rush);
        }
        (rush, furthest)
    };
    let (free_bit, free) = advance(false);
    let (held_bit, held) = advance(true);
    assert!(!free_bit && held_bit, "the bit follows the target's flag while the bodies touch");
    assert!(held < free - 5.0, "bit 15: the root motion at a tenth: {held} cm against {free} cm");
}

/// The sneeze latch is not the status: after the status has run out mid-combo the latched sneeze
/// still plays (controller+0x920 is cleared only when the chain ends).
#[test]
fn the_sneeze_latch_outlives_the_status_until_the_chain_ends() {
    let mut bench = Bench::new(level99(Some(&["COMBO_MASTER"])), &[dummy(1, 150.0, 0.0, 0.0)]);
    bench.sora.take_hit(HitOnPlayer { kind: Kind::Without, ..status_hit(Kind::Small, 1.0, "SNEEZE", 30.0) });
    bench.step(&[], &[]);
    assert_eq!(bench.sora.combat().bad.kind, bad::SNEEZE);
    for _ in 0..400 {
        bench.tap(PadButton::ATTACK);
        bench.step(&[], &[]);
        if bench.sora.combat().sneeze_next {
            break;
        }
    }
    assert!(bench.sora.combat().sneeze_next && bench.sora.combat().history.len() == 3);
    // The status is cured now; the next press is the sneeze all the same.
    bench.sora.combat_mut().bad.time = 0.001;
    bench.idle(2);
    assert_eq!(bench.sora.combat().bad.kind, 0);
    for _ in 0..200 {
        bench.tap(PadButton::ATTACK);
        bench.step(&[], &[]);
        if bench.sora.combat().history.len() >= 4 {
            break;
        }
    }
    assert_eq!(bench.sora.combat().history[3], 0x1E);
}
