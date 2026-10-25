//! The action states, second file (the successor of the actions implementer): Risk Dodge and the
//! Reflect window, the guard reaction, the attack out of a guarded hit, the blow counter
//! (docs/sora-actions-spec.md II.1; combat spec 3.2, 5.6); tier 2: Wall Run, Pole, Rail Slide,
//! Dangle, Slope Slide, Sway, Hopping (II.4 - II.6); tier 3: Floating, Swim.
//! 60 Hz fixed step, the level-99 loadout, `Tuning::GAME_EXACT`, a flat floor at Z = 0 unless a
//! test builds more.

use kh_core::enums::{ETresAbilityKind as Ability, ETresDamageKind as Kind};
use kh_core::host::{EventKind, NullHost, PushBody, ReactionResult, TargetFlags, TargetInfo};
use kh_core::states::HitOnPlayer;
use kh_core::status::LoadoutValue;
use kh_core::test_world::TestWorld;
use kh_core::{surface, HopPoint, Pole, Rail, RailNode};
use kh_core::{AnimId, HitRecord, HitResult, HurtShape, Input, Loadout, Output, PadButton, PadButtons, PadState, Pose, Sora, StateId, Status, TargetId, Targets, Tuning, Vec2, Vec3, SORA_PARAMS};

const DT: f32 = 1.0 / 60.0;
const FORWARD: Vec2 = Vec2::new(0.0, 1.0);

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
    let json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let mut loadout = Loadout::new("sora-loadout-99");
    for row in json["rows"].as_array().unwrap() {
        loadout.set(row[0].as_str().unwrap(), &cell(&row[1]).unwrap()).unwrap();
    }
    loadout.resolve().unwrap();
    Status::from_loadout(&loadout)
}

fn buttons(list: &[PadButton]) -> PadButtons {
    list.iter().fold(PadButtons::NONE, |set, b| set.with(*b))
}

/// An input: the stick, the buttons pressed this step, the buttons held (the pressed ones too).
fn input(stick: Vec2, pressed: &[PadButton], held: &[PadButton]) -> Input {
    let pressed = buttons(pressed);
    Input { move_stick: stick, ..Input::default() }.with_pad(PadState { held: PadButtons(buttons(held).0 | pressed.0), pressed, released: PadButtons::NONE })
}

fn idle() -> Input {
    Input::default()
}

fn stick(direction: Vec2) -> Input {
    input(direction, &[], &[])
}

fn press(button: PadButton, direction: Vec2) -> Input {
    input(direction, &[button], &[])
}

fn near(a: f32, b: f32, tolerance: f32) -> bool {
    (a - b).abs() <= tolerance
}

/// One other pawn of the test: an enemy (or a bystander) the host reports.
#[derive(Clone, Copy)]
struct Foe {
    info: TargetInfo,
    engaged: u32,
    push: Option<u8>,
}

impl Foe {
    fn at(position: Vec3, yaw: f32, radius: f32) -> Foe {
        let flags = TargetFlags(TargetFlags::ALIVE | TargetFlags::HITTABLE | TargetFlags::LOCKABLE | TargetFlags::ENEMY);
        Foe { info: TargetInfo { id: TargetId(1), position, center: position + Vec3::Z * 90.0, velocity: Vec3::ZERO, yaw, radius, half_height: 90.0, flags, hp: 1000.0, hp_max: 1000.0 }, engaged: 0, push: None }
    }
}

impl Targets for Foe {
    fn enumerate(&self, out: &mut Vec<TargetInfo>) {
        out.push(self.info);
    }
    fn info(&self, id: TargetId) -> Option<TargetInfo> {
        (id == self.info.id).then_some(self.info)
    }
    fn hurt_shapes(&self, _id: TargetId, out: &mut Vec<HurtShape>) {
        out.push(HurtShape { a: self.info.center, b: self.info.center, radius: self.info.radius, part: 0 });
    }
    fn apply_hit(&mut self, _hit: &HitRecord) -> HitResult {
        HitResult { accepted: true, ..HitResult::default() }
    }
    fn engaged_count(&self) -> u32 {
        self.engaged
    }
    fn push_body(&self, _id: TargetId) -> Option<PushBody> {
        self.push.map(|level| PushBody { center: self.info.center, radius: self.info.radius, half_height: self.info.half_height, level })
    }
}

struct Bench {
    sora: Sora,
    world: TestWorld,
    foe: Option<Foe>,
    outs: Vec<Output>,
}

impl Bench {
    fn with(world: TestWorld, position: Vec3, yaw: f32) -> Bench {
        let origin = position + Vec3::Z * Sora::origin_height_on_floor(&SORA_PARAMS);
        let sora = Sora::new(SORA_PARAMS, Pose { position: origin, yaw }).with_tuning(Tuning::GAME_EXACT).with_status(level99());
        let mut bench = Bench { sora, world, foe: None, outs: Vec::new() };
        bench.run(5);
        bench
    }

    fn new() -> Bench {
        Bench::with(TestWorld::floor(), Vec3::ZERO, 0.0)
    }

    fn step(&mut self, input: Input) -> Output {
        let mut null = NullHost::default();
        let mut host = null.host(&self.world);
        if let Some(foe) = &mut self.foe {
            host.targets = foe;
        }
        let out = self.sora.step_hosted(DT, &input, &mut host);
        self.outs.push(out.clone());
        out
    }

    fn run(&mut self, steps: usize) -> Output {
        self.hold(idle(), steps)
    }

    fn hold(&mut self, input: Input, steps: usize) -> Output {
        for _ in 0..steps {
            self.step(input);
        }
        self.last().clone()
    }

    /// Steps with `input` until `done`; returns the steps taken (the step that satisfied it is 1).
    fn until(&mut self, input: Input, limit: usize, done: impl Fn(&Output, &Sora) -> bool) -> usize {
        for k in 1..=limit {
            let out = self.step(input);
            if done(&out, &self.sora) {
                return k;
            }
        }
        panic!("not reached within {limit} steps: state {:?} sub {} at {:?} anim {:?}", self.last().state, self.last().sub_step, self.last().position, self.last().anim.name);
    }

    fn last(&self) -> &Output {
        self.outs.last().unwrap()
    }

    fn saw(&self, from: usize, test: impl Fn(&EventKind) -> bool) -> bool {
        self.outs[from..].iter().any(|o| o.events.iter().any(|e| test(&e.kind)))
    }
}

/// A hit of the foe on Sora: `kind`, 30 damage, pushing along the line from the foe.
fn foe_hit(bench: &Bench, kind: Kind, reflect_level: i8) -> HitOnPlayer {
    let foe = bench.foe.expect("a foe");
    let own = bench.last().position;
    let away = ((own - foe.info.position) * Vec3::new(1.0, 1.0, 0.0)).normalize_or_zero();
    HitOnPlayer { direction: away, causer_yaw: foe.info.yaw, reflect_level, ..HitOnPlayer::new(kind, 30.0, foe.info.id, foe.info.position) }
}

// ============================================================================ II.1 Reflect, Risk Dodge

/// Guard at frame N, an attacker 400 cm ahead facing him; its hit with reflect level 3 lands at
/// N + 10 (with or without a risk-dodge offer of `kind`). Returns the bench at frame R (Reflect).
fn guard_broken(kind: Option<u8>) -> Bench {
    let mut bench = Bench::new();
    let z = bench.last().position.z;
    bench.foe = Some(Foe::at(Vec3::new(400.0, 0.0, z), 180.0, 40.0));
    bench.step(press(PadButton::GUARD, Vec2::ZERO));
    bench.run(9);
    if let Some(kind) = kind {
        bench.sora.offer_risk_dodge(TargetId(1), kind, 1.0);
    }
    let hit = foe_hit(&bench, Kind::Small, 3);
    bench.sora.take_hit(hit);
    let out = bench.step(idle());
    assert_eq!((out.state, out.anim.name), (StateId::Reflect, AnimId::from_name("AttackReflect").unwrap()), "II.1 test 11: reflect level 3 breaks the guard into the recoil");
    bench
}

#[test]
fn a_guard_broken_without_an_offer_recoils_400_and_accepts_nothing() {
    // II.1 test 11: speed 400 x 0.95 per tick away from the attacker, no request accepted (jump,
    // guard), Idle when `AttackReflect` (0.9667 s) ends; no risk-dodge prompt.
    let mut bench = guard_broken(None);
    let from = bench.outs.len() - 1;
    let x0 = bench.last().position.x;
    assert!(near(bench.sora.recoil().unwrap().speed, 380.0, 1.0));
    bench.run(5);
    assert_eq!(bench.step(press(PadButton::JUMP, Vec2::ZERO)).state, StateId::Reflect, "II.1 test 13: no offer, the jump is refused");
    assert_eq!(bench.step(press(PadButton::GUARD, Vec2::ZERO)).state, StateId::Reflect);
    let steps = 7 + bench.until(idle(), 80, |o, _| o.state == StateId::Idle);
    assert!(matches!(steps, 57..=60), "the recoil clip is 0.9667 s: {steps}");
    let travelled = x0 - bench.last().position.x;
    assert!(travelled > 60.0 && travelled < 135.0, "about 120 cm back: {travelled}");
    assert!(!bench.saw(from, |e| matches!(e, EventKind::RiskDodgePrompt { .. })));
}

#[test]
fn the_risk_dodge_window_is_0_6_s_and_needs_the_ability() {
    // II.1 "Reflect" Begin / Tick: with an offer of the guarded attacker the prompt shows and the
    // window +0x9C runs 0.6 s; then the prompt hides and the jump is an ordinary refusal.
    let mut bench = guard_broken(Some(1));
    let from = bench.outs.len() - 1;
    assert!(bench.saw(from, |e| matches!(e, EventKind::RiskDodgePrompt { shown: true, .. })) && bench.sora.actions().risk_prompt);
    let steps = bench.until(idle(), 60, |_, s| !s.actions().risk_prompt);
    assert!(matches!(steps, 35..=37), "0.6 s: {steps}");
    assert_eq!(bench.step(press(PadButton::JUMP, Vec2::ZERO)).state, StateId::Reflect, "II.1 test 13: after the window");
    // Without RISKDODGE (8) the offer is not taken at all.
    let mut bench = Bench::new();
    bench.sora.status_mut().abilities.set(Ability::Riskdodge, 1, 0);
    let z = bench.last().position.z;
    bench.foe = Some(Foe::at(Vec3::new(400.0, 0.0, z), 180.0, 40.0));
    bench.step(press(PadButton::GUARD, Vec2::ZERO));
    bench.run(9);
    bench.sora.offer_risk_dodge(TargetId(1), 1, 1.0);
    let hit = foe_hit(&bench, Kind::Small, 3);
    bench.sora.take_hit(hit);
    assert_eq!(bench.step(idle()).state, StateId::Reflect);
    assert!(!bench.sora.actions().risk_prompt);
    bench.run(4);
    assert_eq!(bench.step(press(PadButton::JUMP, Vec2::ZERO)).state, StateId::Reflect);
}

#[test]
fn an_under_slide_ends_behind_the_attacker_facing_it_with_the_counter_offered() {
    // II.1 test 12: type 1 from an attacker 400 cm ahead facing Sora, bounds half-diagonal <= 300:
    // jump at C(R + 5): state 17, reaction type 11 until End, destination = 300 cm behind the
    // attacker (X = 700), root scale (700 - x0) / 800, Sora at the destination facing -X when the
    // root track completes; the world slow and the counter command (reaction command 7, attack
    // kind 0x28) from the tick in which he has passed the attacker until `TresEnableChange`.
    let mut bench = guard_broken(Some(1));
    bench.run(4);
    let x0 = bench.last().position.x;
    let from = bench.outs.len();
    assert_eq!(bench.step(press(PadButton::JUMP, Vec2::ZERO)).state, StateId::Riskdodge);
    assert!(bench.saw(from, |e| matches!(e, EventKind::RiskDodgePrompt { shown: false, used: true })));
    assert_eq!(bench.sora.actions().reaction, 11);
    bench.step(idle());
    let view = bench.sora.risk_dodge().unwrap();
    assert_eq!((view.kind, bench.last().anim.name), (1, AnimId::UnderSlide));
    assert!(near(view.destination.x, 700.0, 0.5) && near(view.root_scale, (700.0 - x0) / 800.0, 1.0e-3), "{view:?} from {x0}");
    // He passes the attacker: slow motion and the offer.
    let passed = bench.until(idle(), 40, |_, s| s.risk_dodge().is_some_and(|v| v.counter_offered));
    assert!(bench.last().position.x > 400.0 && passed < 20, "passed at step {passed}, x {}", bench.last().position.x);
    let offer = bench.sora.combat().guard.offer.expect("the counter offer");
    assert_eq!((offer.kind, offer.source, offer.target), (0x28, StateId::Riskdodge, TargetId(1)));
    assert!(bench.sora.risk_dodge().unwrap().slow_motion && bench.sora.time_dilation(2) < 1.0, "the world slows");
    assert!(bench.outs.iter().skip(from).all(|o| o.state != StateId::Riskdodge || o.events.iter().all(|e| !matches!(e.kind, EventKind::PlayerHit { .. }))));
    // The offer ends at TresEnableChange (0.4667 s of clip time); Idle at the clip end.
    bench.until(idle(), 40, |_, s| s.risk_dodge().is_some_and(|v| !v.counter_offered));
    assert_eq!((bench.sora.combat().guard.offer, bench.sora.time_dilation(2)), (None, 1.0));
    assert_eq!(bench.sora.actions().reaction, 11, "still invincible until End");
    bench.until(idle(), 80, |o, _| o.state == StateId::Idle);
    let out = bench.last().clone();
    // (Setup takes the start position in the controller phase; the Reflect push-back still moves him
    // about 5 cm in that frame's movement tick before Begin stops him: he ends that much short.)
    assert!(near(out.position.x, 700.0, 8.0) && near(out.position.y, 0.0, 3.0), "ended at {:?}", out.position);
    assert!(near(out.yaw.abs(), 180.0, 2.0), "turned to face the attacker: {}", out.yaw);
    assert_eq!((bench.sora.actions().reaction, bench.sora.actions().body_collision_off, bench.sora.actions().risk_dodges), (0, false, 1));
}

#[test]
fn the_under_slide_counter_is_attack_kind_0x28() {
    // Combat spec 5.6.2 "Risk-dodge counter": the attack press while the counter is offered
    // pushes an Attack with the forced kind behind the state; it starts at `TresEnableChange`.
    let mut bench = guard_broken(Some(1));
    bench.run(4);
    bench.step(press(PadButton::JUMP, Vec2::ZERO));
    bench.until(idle(), 40, |_, s| s.risk_dodge().is_some_and(|v| v.counter_offered));
    bench.step(press(PadButton::ATTACK, Vec2::ZERO));
    bench.until(idle(), 60, |o, _| o.state != StateId::Riskdodge);
    assert_eq!(bench.last().state, StateId::Attack);
    assert_eq!(bench.sora.attack().map(|a| a.kind), Some(0x28));
    assert!(bench.sora.combat().guard.counter_used);
}

#[test]
fn slide_turn_over_slide_and_warp_run_to_their_end_invincible() {
    // II.1 types 3, 2 and 4 (flow level): each runs to Idle / a fall with reaction type 11 from
    // Begin to End, offers its counter (0x2A / 0x29 / 0x2B) and leaves him clear of the attacker.
    for (kind, clip, attack) in [(3u8, None, 0x2Au8), (2, Some(AnimId::OverSlide), 0x29), (4, Some(AnimId::ShootFlowStart), 0x2B)] {
        let mut bench = guard_broken(Some(kind));
        bench.run(4);
        assert_eq!(bench.step(press(PadButton::JUMP, Vec2::ZERO)).state, StateId::Riskdodge, "type {kind}");
        bench.step(idle());
        match clip {
            Some(clip) => assert_eq!(bench.last().anim.name, clip),
            None => assert!(matches!(bench.last().anim.name, AnimId::SlideTurnR | AnimId::SlideTurnL)),
        }
        let mut offered = None;
        for _ in 0..200 {
            let out = bench.step(idle());
            if out.state != StateId::Riskdodge {
                break;
            }
            assert_eq!(bench.sora.actions().reaction, 11, "type {kind}: invincible for the whole state");
            if let Some(offer) = bench.sora.combat().guard.offer {
                offered = Some(offer.kind);
            }
        }
        assert_ne!(bench.last().state, StateId::Riskdodge, "type {kind} ends");
        assert_eq!(offered, Some(attack), "type {kind}: its counter kind");
        bench.until(idle(), 200, |o, _| o.state == StateId::Idle);
        let out = bench.last().clone();
        assert!(out.on_floor && (out.position - Vec3::new(400.0, 0.0, out.position.z)).length() > 100.0, "type {kind}: ended at {:?}", out.position);
        assert_eq!((bench.sora.actions().reaction, bench.sora.time_dilation(2)), (0, 1.0));
    }
}

// ============================================================================ guard reaction, plain attack, blow counter

#[test]
fn a_guard_reflect_recoil_of_his_own_attack_is_a_guard_hit_with_the_counter_window() {
    // Combat spec 5.6.3: attacker result GUARD_REFLECT -> the guard reaction (iface vt+0x460
    // 0x141444C30): SetState(15) as a successful hit entry with the 1.0 s counter window.
    let mut bench = Bench::new();
    let z = bench.last().position.z;
    bench.foe = Some(Foe::at(Vec3::new(150.0, 0.0, z), 180.0, 40.0));
    bench.step(press(PadButton::ATTACK, Vec2::ZERO));
    bench.until(idle(), 10, |o, _| o.state == StateId::Attack);
    bench.sora.combat_mut().recoil = Some((ReactionResult::GuardReflect, TargetId(1), Vec3::new(150.0, 0.0, z)));
    bench.until(idle(), 5, |o, _| o.state == StateId::Guard);
    bench.step(idle());
    assert_eq!(bench.last().anim.name, AnimId::GuardHit);
    let a = bench.sora.actions();
    assert!(near(a.counter_window, 1.0, 0.05) && a.counter_attacker == TargetId(1), "window {}", a.counter_window);
    assert_eq!(a.reaction, 3, "a hit entry guards on the body");
    assert!(bench.last().velocity.x < -200.0, "pushed back from the causer: {:?}", bench.last().velocity);
}

#[test]
fn a_plain_attack_leaves_the_guard_hit_clip_after_its_enable_input_without_a_window() {
    // Combat spec 3.2 CanAttack GUARD (0x141426832): without an open counter window the attack
    // is accepted out of a guarded hit from `TresEnableInput` (0.2667 s) and is a plain attack.
    let setup = || {
        let mut bench = Bench::new();
        let z = bench.last().position.z;
        bench.foe = Some(Foe::at(Vec3::new(150.0, 0.0, z), 180.0, 40.0));
        bench.step(press(PadButton::GUARD, Vec2::ZERO));
        bench.run(9);
        let hit = foe_hit(&bench, Kind::Small, 0);
        bench.sora.take_hit(hit);
        assert_eq!(bench.step(idle()).anim.name, AnimId::GuardHit);
        // No window (as without GUARD_COUNTER / the window run out).
        bench.sora.actions_mut().counter_window = 0.0;
        bench.sora.actions_mut().counter_command = 0;
        bench
    };
    let mut bench = setup();
    bench.run(10);
    bench.step(press(PadButton::ATTACK, Vec2::ZERO));
    bench.run(2);
    assert_eq!(bench.last().state, StateId::Guard, "before TresEnableInput: refused");
    let mut bench = setup();
    bench.run(17);
    bench.step(press(PadButton::ATTACK, Vec2::ZERO));
    bench.until(idle(), 4, |o, _| o.state == StateId::Attack);
    let kind = bench.sora.attack().unwrap().kind;
    assert!(!matches!(kind, 0x21 | 0x23 | 0x24 | 0x25), "a plain attack, not a counter: kind {kind:#x}");
}

#[test]
fn an_attack_out_of_an_aerial_recovery_is_the_blow_counter() {
    // Combat spec 9.5 / actions II.2 "Aerial Recovery": BLOW_COUNTER arms reaction command 2;
    // the attack press during the recovery pushes an Attack with the forced kind 0x27.
    let mut bench = Bench::new();
    let z = bench.last().position.z;
    bench.foe = Some(Foe::at(Vec3::new(150.0, 0.0, z), 180.0, 40.0));
    let hit = foe_hit(&bench, Kind::Blow, 0);
    bench.sora.take_hit(hit);
    bench.step(idle());
    assert_eq!(bench.last().state, StateId::Damage);
    bench.until(idle(), 40, |_, sora| sora.damage_reaction().is_some_and(|d| d.recoverable));
    assert_eq!(bench.step(press(PadButton::JUMP, Vec2::ZERO)).state, StateId::AirSlide);
    assert_eq!(bench.sora.actions().reaction_command.map(|c| c.0), Some(2));
    bench.run(2);
    bench.step(press(PadButton::ATTACK, Vec2::ZERO));
    bench.until(idle(), 40, |o, _| o.state == StateId::Attack);
    assert_eq!(bench.sora.attack().map(|a| a.kind), Some(0x27), "Payback Strike");
    assert_eq!(bench.sora.actions().reaction_command, None);
}


// ============================================================================ tier 2: II.4 Wall Run

const RIGHT: Vec2 = Vec2::new(1.0, 0.0);

/// The stick pushed fully in the world direction `yaw` (degrees; the camera yaw is 0).
fn toward(yaw: f32) -> Vec2 {
    let a = yaw.to_radians();
    Vec2::new(a.sin(), a.cos())
}

/// A flat floor and a runnable wall: its face at x = 500, 30 m wide, `height` high.
fn wall_world(height: f32) -> TestWorld {
    TestWorld::floor().with_box(Vec3::new(500.0, -1500.0, 0.0), Vec3::new(700.0, 1500.0, height)).flagged(surface::WALL_RUN)
}

#[test]
fn running_at_a_flagged_wall_runs_up_it_at_1251_and_letting_go_ends_it() {
    // II.4 test 1: run straight at the wall: Run's world hit -> WALLRUN, class 0, facing
    // yaw(N) + 180, Flying, `WallRunF`; with the stick held the loop from TresEnableChange
    // (0.1671 s), then 1251 cm/s upward (20.85 cm per tick). Releasing the stick: the run ends
    // 12 - 13 ticks later with half the actual velocity (about 626 cm/s up), mode Falling.
    let mut bench = Bench::with(wall_world(3000.0), Vec3::ZERO, 0.0);
    let run = stick(FORWARD);
    bench.until(run, 120, |o, _| o.state == StateId::Wallrun);
    bench.step(run);
    assert_eq!((bench.last().anim.name, bench.last().yaw.round()), (AnimId::WallRunF, 0.0));
    bench.until(run, 30, |o, _| o.anim.name == AnimId::WallRunLoopF);
    bench.hold(run, 10);
    let (z0, z1) = (bench.outs[bench.outs.len() - 4].position.z, bench.last().position.z);
    assert!(near((z1 - z0) / 3.0, 1251.2 / 60.0, 0.5), "per tick {}", (z1 - z0) / 3.0);
    assert!(bench.last().position.x < 440.0 + 1.0, "he stays on the wall face: {:?}", bench.last().position);
    let steps = bench.until(idle(), 30, |o, _| o.state != StateId::Wallrun);
    assert!(matches!(steps, 12..=14), "the 0.2 s input grace: {steps}");
    let out = bench.last().clone();
    assert!(out.state == StateId::Jump && near(out.velocity.z, 625.6, 40.0), "{:?} vz {}", out.state, out.velocity.z);
    assert_eq!(bench.sora.actions().traversal.begun[0], 1);
    // Without the flag (a plain wall) and with the ability mask's WallRun bit cleared: nothing.
    let mut bench = Bench::with(TestWorld::floor().with_box(Vec3::new(500.0, -1500.0, 0.0), Vec3::new(700.0, 1500.0, 3000.0)), Vec3::ZERO, 0.0);
    bench.hold(run, 120);
    assert_ne!(bench.last().state, StateId::Wallrun);
    let mut bench = Bench::with(wall_world(3000.0), Vec3::ZERO, 0.0);
    bench.sora.actions_mut().traversal.wall_run_unlocked = false;
    bench.hold(run, 120);
    assert_ne!(bench.last().state, StateId::Wallrun);
}

#[test]
fn the_stick_angle_picks_up_run_side_run_or_nothing() {
    // II.4 test 2: 40 degrees off the wall direction: no state change (the dead zone 35 - 50);
    // 60 degrees: a side run on that side, facing along the wall, 1251 cm/s after the entry clip.
    let mut bench = Bench::with(wall_world(3000.0), Vec3::new(250.0, 0.0, 0.0), 0.0);
    bench.hold(stick(toward(40.0)), 100);
    assert_ne!(bench.last().state, StateId::Wallrun, "40 degrees: the dead zone");
    let mut bench = Bench::with(wall_world(3000.0), Vec3::new(250.0, 0.0, 0.0), 0.0);
    let side = stick(toward(60.0));
    bench.until(side, 150, |o, _| o.state == StateId::Wallrun);
    bench.step(side);
    // The stick points to +Y of the wall direction: the run goes toward +Y, the wall on his left.
    assert_eq!((bench.last().anim.name, bench.last().yaw.round()), (AnimId::WallRunL, 90.0));
    bench.until(stick(toward(90.0)), 60, |o, _| o.anim.name == AnimId::WallRunLoopL);
    bench.hold(stick(toward(90.0)), 12);
    let (a, b) = (bench.outs[bench.outs.len() - 4].position, bench.last().position);
    assert!(near((b.y - a.y) / 3.0, 1251.2 / 60.0, 0.6) && near(b.z, a.z, 1.0), "along the wall, level: {a:?} -> {b:?}");
}

#[test]
fn a_wall_run_jumps_off_with_a_super_jump_and_kicks_off_with_the_guard_button() {
    // II.4 tests 6 and 8: the jump press in the loop: Jump type 2 (needs SUPERJUMP); the guard
    // press (needs SUPERSLIDE): WALL_KICK after the probe, then a Superslide at the kick's end.
    let on_the_wall = || {
        let mut bench = Bench::with(wall_world(3000.0), Vec3::ZERO, 0.0);
        bench.until(stick(FORWARD), 120, |o, _| o.anim.name == AnimId::WallRunLoopF);
        bench.hold(stick(FORWARD), 5);
        bench
    };
    let mut bench = on_the_wall();
    bench.step(press(PadButton::JUMP, FORWARD));
    bench.until(stick(FORWARD), 3, |o, _| o.state == StateId::Jump);
    assert_eq!(bench.sora.jump_kind(), Some(2));
    let mut bench = on_the_wall();
    bench.sora.status_mut().abilities.set(Ability::Superjump, 1, 0);
    bench.step(press(PadButton::JUMP, FORWARD));
    bench.step(stick(FORWARD));
    assert_eq!(bench.last().state, StateId::Wallrun, "without SUPERJUMP: no effect");
    let mut bench = on_the_wall();
    bench.step(press(PadButton::GUARD, FORWARD));
    bench.until(stick(FORWARD), 3, |o, _| o.state == StateId::WallKick);
    bench.until(idle(), 60, |o, _| o.state != StateId::WallKick);
    assert_eq!(bench.last().state, StateId::SuperSlide, "the kick ends in a Superslide");
}

// ============================================================================ tier 2: II.4 Pole

fn pole(horizontal: bool) -> Pole {
    if horizontal {
        Pole { id: 7, base1: Vec3::new(900.0, -200.0, 500.0), base2: Vec3::new(900.0, 200.0, 500.0), check_radius: 50.0, attach_radius: 30.0, horizontal: true, active: true }
    } else {
        Pole { id: 7, base1: Vec3::new(900.0, 0.0, 150.0), base2: Vec3::new(900.0, 0.0, 1100.0), check_radius: 100.0, attach_radius: 90.0, horizontal: false, active: true }
    }
}

/// Jumps and air-slides along +X toward the pole; returns at the first PoleTurn step.
fn slide_onto(pole: Pole) -> Bench {
    let mut world = TestWorld::floor();
    world.poles.push(pole);
    let mut bench = Bench::with(world, Vec3::ZERO, 0.0);
    bench.step(press(PadButton::JUMP, Vec2::ZERO));
    bench.until(input(Vec2::ZERO, &[], &[PadButton::JUMP]), 120, |o, _| o.state == StateId::Jump && o.sub_step == 4);
    assert_eq!(bench.step(press(PadButton::GUARD, FORWARD)).state, StateId::AirSlide);
    bench.until(idle(), 60, |o, _| o.state == StateId::PoleTurn);
    bench
}

#[test]
fn a_vertical_pole_is_caught_from_an_air_slide_spins_1_2_s_and_launches_a_superslide() {
    // II.4 test 9: the point 120 ahead / 90 up within 100 + m_CheckRadius of the segment:
    // POLE_TURN; `PoleTurn` 0.1667 s, then `PoleTurnLoop`, 360 degrees per 0.3 s; after 1.2 s of
    // loop without input a Superslide starts by itself; the same pole is not caught again by it.
    let mut bench = slide_onto(pole(false));
    assert!(bench.last().position.x > 500.0 && bench.last().position.x < 800.0, "caught at {:?}", bench.last().position);
    bench.step(idle());
    assert_eq!(bench.last().anim.name, AnimId::PoleTurn);
    let entry = 1 + bench.until(idle(), 20, |o, _| o.anim.name == AnimId::PoleTurnLoop);
    assert!(matches!(entry, 10..=12), "the 0.1667 s entry clip: {entry}");
    // On the pole: `m_AttachRadius` from the axis, turning 20 degrees per tick.
    bench.run(3);
    let (a, b) = (bench.outs[bench.outs.len() - 2].clone(), bench.last().clone());
    assert!(near((b.position - Vec3::new(900.0, 0.0, b.position.z)).length(), 90.0, 1.0), "{:?}", b.position);
    assert!(near(kh_core::sora::wrap_degrees(b.yaw - a.yaw), 20.0, 0.5), "{} -> {}", a.yaw, b.yaw);
    let spin = 3 + bench.until(idle(), 100, |o, _| o.state != StateId::PoleTurn);
    assert!(matches!(spin, 72..=75), "4 loops of 0.3 s: {spin}");
    assert_eq!(bench.last().state, StateId::SuperSlide);
    bench.run(30);
    assert_eq!(bench.last().state, StateId::SuperSlide, "the pole just left is not caught again");
    // The jump button: a directed super jump at once.
    let mut bench = slide_onto(pole(false));
    bench.until(idle(), 20, |o, _| o.anim.name == AnimId::PoleTurnLoop);
    bench.run(4);
    bench.step(press(PadButton::JUMP, Vec2::ZERO));
    bench.until(idle(), 4, |o, _| o.state == StateId::Jump);
    assert_eq!(bench.sora.jump_kind(), Some(2));
    // Without POLE_SPIN (95) the pole is ignored.
    let mut world = TestWorld::floor();
    world.poles.push(pole(false));
    let mut bench = Bench::with(world, Vec3::ZERO, 0.0);
    bench.sora.status_mut().abilities.set(Ability::PoleSpin, 1, 0);
    bench.step(press(PadButton::JUMP, Vec2::ZERO));
    bench.until(input(Vec2::ZERO, &[], &[PadButton::JUMP]), 120, |o, _| o.state == StateId::Jump && o.sub_step == 4);
    bench.step(press(PadButton::GUARD, FORWARD));
    bench.run(60);
    assert!(bench.outs.iter().all(|o| o.state != StateId::PoleTurn));
}

#[test]
fn a_horizontal_bar_swings_1_6_s_and_then_jumps_by_itself() {
    // II.4 test 10: `PoleSwing` / `PoleSwingLoop` (0.4 s); after 1.6 s an automatic directed
    // jump; the guard press: a Superslide.
    let mut bench = slide_onto(pole(true));
    bench.until(idle(), 20, |o, _| o.anim.name == AnimId::PoleSwingLoop);
    assert!(near((bench.last().position - Vec3::new(900.0, 0.0, 500.0)).length(), 0.0, 1.0), "at the bar: {:?}", bench.last().position);
    let swing = bench.until(idle(), 120, |o, _| o.state != StateId::PoleTurn);
    assert!(matches!(swing, 95..=99), "4 loops of 0.4 s: {swing}");
    assert_eq!((bench.last().state, bench.sora.jump_kind()), (StateId::Jump, Some(2)));
    let mut bench = slide_onto(pole(true));
    bench.until(idle(), 20, |o, _| o.anim.name == AnimId::PoleSwingLoop);
    bench.step(press(PadButton::GUARD, Vec2::ZERO));
    bench.until(idle(), 4, |o, _| o.state == StateId::SuperSlide);
}

// ============================================================================ tier 2: II.5 Rail Slide

/// A straight level rail along +X at the height of his origin on the floor, 3000 cm long.
fn rail_world() -> (TestWorld, f32) {
    let z = Sora::origin_height_on_floor(&SORA_PARAMS);
    let node = |x: f32| RailNode { position: Vec3::new(x, 0.0, z), speed: None, detect_range_plus: 0.0 };
    let mut world = TestWorld::floor();
    world.rails.push(Rail { id: 3, nodes: vec![node(0.0), node(3000.0)], start_dir: 0, no_reverse: false, no_jump: false, narrow: false });
    (world, z)
}

/// Sora dropped beside the rail at x = 1000 (120 cm to its side), then stepping onto it.
fn on_the_rail() -> Bench {
    let (world, _) = rail_world();
    let mut bench = Bench::with(world, Vec3::new(1000.0, 120.0, 0.0), 0.0);
    assert_eq!(bench.last().state, StateId::Idle, "120 cm away: out of the ground range 50");
    bench.until(stick(toward(-70.0)), 60, |o, _| o.state == StateId::RailSlide);
    bench
}

#[test]
fn a_rail_is_taken_from_the_ground_within_50_cm_and_slides_at_2000() {
    // II.5 Rail 1 / Rail 2: the probe hits within 50 cm; state 22: facing snapped to the rail,
    // `RailSlideStart` with the movement paused (0.2333 s = 14 ticks), then 33.33 cm per tick;
    // at the end of the rail a super jump with the launch block, and the rail is not re-taken.
    let mut bench = on_the_rail();
    bench.step(idle());
    let attach = bench.last().clone();
    assert_eq!(attach.anim.name, AnimId::RailSlideStart);
    assert!(near(attach.position.y, 0.0, 0.01) && near(attach.yaw, 0.0, 0.5), "snapped onto the rail: {:?} yaw {}", attach.position, attach.yaw);
    let paused = 1 + bench.until(idle(), 30, |o, _| o.anim.name == AnimId::RailSlideLoop);
    assert!(matches!(paused, 14..=16), "{paused}");
    bench.run(6);
    let (a, b) = (bench.outs[bench.outs.len() - 4].position, bench.last().position);
    assert!(near((b.x - a.x) / 3.0, 2000.0 / 60.0, 0.01) && near(b.y, 0.0, 0.01), "{a:?} -> {b:?}");
    bench.until(idle(), 120, |o, _| o.state != StateId::RailSlide);
    assert!(bench.outs[bench.outs.len() - 2].position.x > 2960.0, "left at the end: {:?}", bench.outs[bench.outs.len() - 2].position);
    assert_eq!((bench.last().state, bench.sora.jump_kind()), (StateId::Jump, Some(2)));
    bench.run(8);
    assert!(bench.outs.iter().rev().take(8).all(|o| o.state == StateId::Jump), "the rail just left is refused");
    // The FreeFlow bit off: no rail.
    let (world, _) = rail_world();
    let mut bench = Bench::with(world, Vec3::new(1000.0, 120.0, 0.0), 0.0);
    bench.sora.actions_mut().flow_unlocked = false;
    bench.hold(stick(toward(-70.0)), 60);
    assert!(bench.outs.iter().all(|o| o.state != StateId::RailSlide));
}

#[test]
fn on_a_rail_the_stick_turns_him_round_and_the_buttons_leave_it() {
    // II.5 Rail 3 - 5: the stick against the travel direction: reversed on the same tick,
    // `RailSlideTurn` (0.3 s, paused), then 2000 cm/s the other way; not while the stick has been
    // held since the attach (bit 14). Jump: a super jump along the rail. Guard: a Superslide.
    let mut bench = on_the_rail();
    bench.until(idle(), 30, |o, _| o.anim.name == AnimId::RailSlideLoop);
    bench.run(36);
    let before = bench.last().position.x;
    bench.until(stick(Vec2::new(0.0, -1.0)), 4, |o, _| o.anim.name == AnimId::RailSlideTurn);
    assert!(near(bench.last().yaw.abs(), 180.0, 1.0));
    bench.until(stick(Vec2::new(0.0, -1.0)), 30, |o, _| o.anim.name == AnimId::RailSlideLoop);
    bench.hold(stick(Vec2::new(0.0, -1.0)), 6);
    let (a, b) = (bench.outs[bench.outs.len() - 4].position, bench.last().position);
    assert!(near((a.x - b.x) / 3.0, 2000.0 / 60.0, 0.01) && b.x < before, "back toward the first node: {a:?} -> {b:?}");
    // Still holding: no second turn until the stick has been released once.
    bench.hold(stick(FORWARD), 30);
    assert!(near(bench.last().yaw.abs(), 180.0, 1.0), "the stick must be released first... {}", bench.last().yaw);
    // Jump off.
    let mut bench = on_the_rail();
    bench.until(idle(), 30, |o, _| o.anim.name == AnimId::RailSlideLoop);
    bench.run(3);
    bench.step(press(PadButton::JUMP, Vec2::ZERO));
    bench.until(idle(), 3, |o, _| o.state == StateId::Jump);
    assert_eq!(bench.sora.jump_kind(), Some(2));
    // Guard off.
    let mut bench = on_the_rail();
    bench.until(idle(), 30, |o, _| o.anim.name == AnimId::RailSlideLoop);
    bench.run(3);
    bench.step(press(PadButton::GUARD, Vec2::ZERO));
    bench.until(idle(), 3, |o, _| o.state == StateId::SuperSlide);
}

// ============================================================================ tier 2: II.6 Dangle

/// A 3 m block whose face is at x = 300, hangable; Sora stands 62 cm in front of it. (A 2 m ledge
/// over flat ground is NOT hangable: the SPACE test's column of radius 50 down to 10 cm below the
/// hanging feet meets the floor.)
fn ledge_bench() -> Bench {
    let world = TestWorld::floor().with_box(Vec3::new(300.0, -300.0, 0.0), Vec3::new(900.0, 300.0, 300.0)).flagged(surface::DANGLE);
    Bench::with(world, Vec3::new(238.0, 0.0, 0.0), 0.0)
}

/// Jumps on the spot; returns hanging (the first Dangle step).
fn hang() -> Bench {
    let mut bench = ledge_bench();
    bench.step(press(PadButton::JUMP, Vec2::ZERO));
    bench.until(idle(), 200, |o, _| o.state == StateId::Dangle);
    bench
}

#[test]
fn a_ledge_is_grabbed_in_the_jump_and_the_stick_climbs_or_lets_go() {
    // II.6 "Dangle": the feet 150 cm below the ledge top and 60 cm back from the edge, facing
    // the wall, no movement (mode Custom); `GripStart`; from TresEnableChange (0.199 s) the stick
    // toward the wall climbs (`GripRise`: he ends on the top), away from it lets go (0.6 s
    // cool-down); the jump button drops him.
    let mut bench = hang();
    let out = bench.step(idle());
    assert_eq!(out.anim.name, AnimId::GripStart);
    assert!(near(out.position.z, 150.0, 0.5) && near(out.position.x, 240.0, 0.5) && near(out.yaw, 0.0, 0.5), "hangs at {:?} yaw {}", out.position, out.yaw);
    bench.run(30);
    assert!((bench.last().position - out.position).length() < 0.01 && bench.last().velocity == Vec3::ZERO, "he does not move");
    // Climb.
    bench.until(stick(FORWARD), 3, |o, _| o.anim.name == AnimId::GripRise);
    bench.until(idle(), 120, |o, _| o.state == StateId::Idle && o.on_floor);
    assert!(near(bench.last().position.z, 300.15, 1.0) && bench.last().position.x > 320.0, "on the ledge: {:?}", bench.last().position);
    // Too early: before TresEnableChange the stick does nothing.
    let mut bench = hang();
    bench.hold(stick(FORWARD), 5);
    assert_eq!(bench.last().anim.name, AnimId::GripStart);
    // Sora is pad-driven (the tick's vt+0x1A20 branch, 0x14150F4D8 / 0x14150F4E1): a stick 50, 90
    // or 120 degrees off the wall neither climbs nor lets go (between 45 and 135 he stays). (party I1)
    let mut bench = hang();
    bench.run(20);
    let at = bench.last().position;
    for dir in [Vec2::new(1.0, 0.0), Vec2::new(50f32.to_radians().sin(), 50f32.to_radians().cos()), Vec2::new(120f32.to_radians().sin(), 120f32.to_radians().cos())] {
        bench.hold(stick(dir), 10);
        assert_eq!(bench.last().state, StateId::Dangle, "stays at stick {dir:?}");
        assert!((bench.last().position - at).length() < 0.01);
    }
    // 40 degrees off climbs.
    bench.until(stick(Vec2::new(40f32.to_radians().sin(), 40f32.to_radians().cos())), 3, |o, _| o.anim.name == AnimId::GripRise);
    // Let go (135 degrees or more off): falls, and the ledge is not grabbed again for 0.6 s.
    let mut bench = hang();
    bench.run(20);
    bench.until(stick(Vec2::new(0.0, -1.0)), 3, |o, _| o.state == StateId::Jump);
    assert!(bench.sora.actions().traversal.dangle_cooldown > 0.5);
    bench.until(idle(), 60, |o, _| o.on_floor);
    assert!(bench.outs.iter().rev().take(20).all(|o| o.state != StateId::Dangle));
    // The jump button drops him.
    let mut bench = hang();
    bench.run(20);
    bench.step(press(PadButton::JUMP, Vec2::ZERO));
    bench.until(idle(), 3, |o, _| o.state == StateId::Jump && o.sub_step >= 3);
    assert!(bench.last().velocity.z <= 0.0, "a drop, no launch");
    // A ledge that is not flagged is not grabbed.
    let world = TestWorld::floor().with_box(Vec3::new(300.0, -300.0, 0.0), Vec3::new(900.0, 300.0, 300.0));
    let mut bench = Bench::with(world, Vec3::new(238.0, 0.0, 0.0), 0.0);
    bench.step(press(PadButton::JUMP, Vec2::ZERO));
    bench.run(150);
    assert!(bench.outs.iter().all(|o| o.state != StateId::Dangle));
}

// ============================================================================ tier 2: II.6 Slope Slide

#[test]
fn landing_on_a_slide_floor_slides_to_1600_and_stops_on_the_flat() {
    // II.6 "Slope Slide": a floor flagged SQEX_bEnableSlopeSlide descending at 30 degrees:
    // landing on it always slides; the speed ramps to 1600 cm/s within 0.5 s; on the flat the
    // speed drops and, without the stick, he stops with `LightLandR` under 680 cm/s.
    let slope = || {
        let n = Vec3::new(30.0f32.to_radians().sin(), 0.0, 30.0f32.to_radians().cos());
        TestWorld::floor().with_plane_through(Vec3::new(0.0, 0.0, 1500.0), n).with_flags(2, surface::WALL_KICK | surface::SLOPE_SLIDE).with_flags(1, surface::WALL_KICK | surface::SLOPE_SLIDE)
    };
    let mut bench = Bench::with(slope(), Vec3::new(0.0, 0.0, 1560.0), 0.0);
    bench.until(idle(), 60, |o, _| o.state == StateId::Slopeslide);
    bench.step(idle());
    assert_eq!(bench.last().anim.name, AnimId::SlopeSlide);
    let mut fastest = 0.0f32;
    for _ in 0..40 {
        let out = bench.step(idle());
        assert_eq!(out.state, StateId::Slopeslide);
        fastest = fastest.max(out.velocity.truncate().length());
    }
    assert!(near(fastest, 1600.0, 1.0), "the ramp ends at 1600: {fastest}");
    assert!(bench.last().position.z < 1300.0 && bench.last().on_floor, "down the slope: {:?}", bench.last().position);
    bench.until(idle(), 400, |o, _| o.anim.name == AnimId::LightLandR);
    assert!(bench.last().position.z < 5.0, "on the flat: {:?}", bench.last().position);
    bench.until(idle(), 60, |o, _| o.state == StateId::Idle);
    // Standing on the same slope without a previous slide: no slide (the latch is clear).
    let mut bench = Bench::with(slope(), Vec3::new(0.0, 0.0, 1500.0 + 0.2), 0.0);
    bench.run(60);
    assert!(bench.outs.iter().all(|o| o.state != StateId::Slopeslide), "standing on it does not start a slide");
    // With the stick held on the flat he breaks into a run.
    let mut bench = Bench::with(slope(), Vec3::new(0.0, 0.0, 1560.0), 0.0);
    bench.until(idle(), 60, |o, _| o.state == StateId::Slopeslide);
    bench.until(stick(FORWARD), 500, |o, _| o.state == StateId::Run);
    // An unflagged slope: no slide.
    let n = Vec3::new(30.0f32.to_radians().sin(), 0.0, 30.0f32.to_radians().cos());
    let mut bench = Bench::with(TestWorld::floor().with_plane_through(Vec3::new(0.0, 0.0, 1500.0), n), Vec3::new(0.0, 0.0, 1560.0), 0.0);
    bench.run(120);
    assert!(bench.outs.iter().all(|o| o.state != StateId::Slopeslide));
}

// ============================================================================ tier 2: II.6 Swaying

#[test]
fn running_into_a_pawn_side_steps_round_it() {
    // II.6 "Swaying": a pawn within 50 degrees of the facing, bumped at full run speed with the
    // stick above 0.5, not in battle: `RunSwayL` (the pawn dead ahead counts as "to the right"),
    // full run speed along the facing plus the clip's lateral arc; stick held: to the clip end
    // (0.9 s) and on into Run; released: it ends at TresEnableChange (0.39 s).
    let setup = || {
        let mut bench = Bench::new();
        let z = bench.last().position.z;
        let mut foe = Foe::at(Vec3::new(700.0, 0.0, z), 180.0, 40.0);
        foe.info.flags = TargetFlags(TargetFlags::ALIVE);
        bench.foe = Some(foe);
        bench.until(stick(FORWARD), 120, |o, _| o.state == StateId::Swaying);
        bench
    };
    let mut bench = setup();
    assert!(bench.last().position.x > 590.0 && bench.last().position.x < 615.0, "at the contact: {:?}", bench.last().position);
    bench.step(stick(FORWARD));
    assert_eq!(bench.last().anim.name, AnimId::RunSwayL);
    let mut widest = 0.0f32;
    let mut steps = 1;
    while bench.last().state == StateId::Swaying && steps < 80 {
        let out = bench.step(stick(FORWARD));
        widest = widest.max(out.position.y.abs());
        steps += 1;
    }
    assert!(matches!(steps, 53..=56), "the 0.9 s clip: {steps}");
    assert!(widest > 90.0 && widest < 115.0, "the arc is about 109.5 cm wide: {widest}");
    assert_eq!(bench.last().state, StateId::Run);
    assert!(bench.last().position.x > 1100.0, "he kept his run speed: {:?}", bench.last().position);
    // Stick released: the state ends at TresEnableChange.
    let mut bench = setup();
    let steps = bench.until(idle(), 40, |o, _| o.state != StateId::Swaying);
    assert!(matches!(steps, 23..=26), "0.39 s: {steps}");
    // In battle: no side-step.
    let mut bench = Bench::new();
    let z = bench.last().position.z;
    let mut foe = Foe::at(Vec3::new(700.0, 0.0, z), 180.0, 40.0);
    foe.engaged = 1;
    bench.foe = Some(foe);
    bench.hold(stick(FORWARD), 120);
    assert!(bench.outs.iter().all(|o| o.state != StateId::Swaying));
}

// ============================================================================ tier 2: II.5 Hopping

fn hop_world(points: &[(u32, f32)]) -> TestWorld {
    let z = Sora::origin_height_on_floor(&SORA_PARAMS) + 100.0;
    let mut world = TestWorld::floor();
    for &(id, x) in points {
        world.hops.push(HopPoint { id, position: Vec3::new(x, 0.0, z), check_radius: 100.0, idle_motion: 1, end_point: false, end_roll: false, active: true });
    }
    world
}

#[test]
fn a_hop_point_in_reach_pulls_him_on_and_the_stick_hops_to_the_next_or_off() {
    // II.5 Hop 1 - 4: standing within 100 + m_CheckRadius of a point: state 9 from Idle's Begin;
    // `HopL`, a straight move at 600 cm/s or with the clip, then `BalanceL` on the point (mode
    // Custom: no movement); the stick above 0.85 finds the next point 150 - 350 cm ahead (`HopR`);
    // without one he hops off forward and falls.
    // (The state he spawns in has had no Begin: a guard, and the Idle that follows it probes.)
    let mut bench = Bench::with(hop_world(&[(1, 500.0), (2, 800.0)]), Vec3::new(330.0, 0.0, 0.0), 0.0);
    bench.step(press(PadButton::GUARD, Vec2::ZERO));
    bench.until(idle(), 70, |o, _| o.state == StateId::Hopping);
    bench.step(idle());
    assert_eq!(bench.last().anim.name, AnimId::HopL);
    let target = Vec3::new(500.0, 0.0, Sora::origin_height_on_floor(&SORA_PARAMS) + 100.0);
    bench.until(idle(), 40, |o, _| o.anim.name == AnimId::BalanceL);
    assert!((bench.last().position - target).length() < 0.01, "on the point: {:?}", bench.last().position);
    bench.run(30);
    assert!((bench.last().position - target).length() < 0.01 && bench.last().state == StateId::Hopping);
    // Half tilt: he only turns.
    bench.hold(stick(FORWARD * 0.7), 10);
    assert_eq!(bench.last().anim.name, AnimId::BalanceL);
    // Full tilt: the next point.
    bench.step(stick(FORWARD));
    bench.until(stick(FORWARD), 3, |o, _| o.anim.name == AnimId::HopR);
    bench.until(idle(), 40, |o, _| o.anim.name == AnimId::BalanceR);
    assert!(near(bench.last().position.x, 800.0, 0.01), "{:?}", bench.last().position);
    assert_eq!(bench.sora.actions().traversal.begun[6], 2);
    // No further point: he hops off forward at walk speed and falls.
    bench.hold(stick(FORWARD), 3);
    assert!(matches!(bench.last().anim.name, AnimId::HopL | AnimId::HopR) && bench.last().sub_step == 3);
    bench.until(idle(), 60, |o, _| o.state == StateId::Jump);
    assert!(bench.last().position.x > 900.0, "{:?}", bench.last().position);
    // Out of reach (210 cm from the point): nothing.
    let mut bench = Bench::with(hop_world(&[(1, 500.0)]), Vec3::new(285.0, 0.0, 0.0), 0.0);
    bench.step(press(PadButton::GUARD, Vec2::ZERO));
    bench.run(80);
    assert!(bench.outs.iter().all(|o| o.state != StateId::Hopping));
    // The jump button jumps off the point.
    let mut bench = Bench::with(hop_world(&[(1, 500.0)]), Vec3::new(330.0, 0.0, 0.0), 0.0);
    bench.step(press(PadButton::GUARD, Vec2::ZERO));
    bench.until(idle(), 120, |o, _| o.anim.name == AnimId::BalanceL);
    bench.step(press(PadButton::JUMP, Vec2::ZERO));
    bench.until(idle(), 3, |o, _| o.state == StateId::Jump);
}

#[allow(dead_code)]
fn unused_helpers() {
    let _ = (RIGHT, RailNode { position: Vec3::ZERO, speed: None, detect_range_plus: 0.0 });
}

// ============================================================================ tier 3: II.2 Floating

#[test]
fn the_floating_mode_hangs_him_flies_with_the_stick_and_coasts() {
    // II.2 "Floating": the host's switch (`BP_NotifySetFloatingMode`): "on ground" is false, Idle
    // hands over to FLOATING: Falling at gravity scale 0.01 (`GlideIdle`); the stick above 0.5:
    // `GlideLoop`, 680 cm/s along a heading that lags the stick; released: he coasts, x 0.98 per
    // tick; the jump button is a Doubleflight; the switch off: he falls and lands.
    let mut bench = Bench::new();
    bench.step(press(PadButton::JUMP, Vec2::ZERO));
    bench.run(25);
    bench.sora.set_floating(true, false);
    bench.until(idle(), 120, |o, _| o.state == StateId::Floating); // (from the fall on: a rising jump goes on)
    bench.run(2);
    assert_eq!(bench.last().anim.name, AnimId::GlideIdle);
    let before = bench.last().clone();
    bench.run(30);
    let sink = bench.outs[bench.outs.len() - 2].velocity.z - bench.last().velocity.z;
    assert!(near(sink, 980.0 * 0.01 / 60.0, 0.01), "gravity scale 0.01: {sink} per tick");
    assert!(!bench.last().on_floor && (before.position.truncate() - bench.last().position.truncate()).length() < 1.0);
    bench.hold(stick(FORWARD), 40);
    assert_eq!(bench.last().anim.name, AnimId::GlideLoop);
    assert!(near(bench.last().velocity.truncate().length(), 680.0, 5.0), "{:?}", bench.last().velocity);
    bench.run(2);
    assert_eq!(bench.last().anim.name, AnimId::GlideIdle);
    let v0 = bench.last().velocity.truncate().length();
    bench.run(10);
    let v1 = bench.last().velocity.truncate().length();
    assert!(near(v1 / v0, 0.98f32.powi(10), 0.01), "coasting: {v0} -> {v1}");
    // Jump out: a Doubleflight.
    bench.step(press(PadButton::JUMP, Vec2::ZERO));
    bench.until(idle(), 3, |o, _| o.state == StateId::Jump);
    assert_eq!(bench.sora.jump_kind(), Some(3));
    // ... and back into the mode, until the host switches it off.
    bench.until(idle(), 120, |o, _| o.state == StateId::Floating);
    bench.sora.set_floating(false, false);
    bench.until(idle(), 400, |o, _| o.state == StateId::Idle && o.on_floor);
}

// ============================================================================ ability audit: the two untested defence abilities

#[test]
fn block_replenisher_gives_3_mp_for_a_guarded_hit() {
    // GUARD_REGENE (74, Block Replenisher; II.1 `OnGuardedHit` 0x14143D5EA): pawn.vt+0x1878(0, 3, 0,
    // 0, 0) = MP +3 on a guarded hit; nothing without the ability.
    let guarded = |equipped: bool| {
        let mut bench = Bench::new();
        let z = bench.last().position.z;
        bench.foe = Some(Foe::at(Vec3::new(150.0, 0.0, z), 180.0, 40.0));
        if !equipped {
            bench.sora.status_mut().abilities.set(Ability::GuardRegene, 1, 0);
        }
        let full = bench.sora.status().mp;
        bench.sora.status_mut().set_mp(full - 20);
        bench.step(press(PadButton::GUARD, Vec2::ZERO));
        bench.run(9);
        let hit = foe_hit(&bench, Kind::Small, 0);
        bench.sora.take_hit(hit);
        bench.step(idle());
        assert_eq!(bench.last().state, StateId::Guard);
        bench.sora.status().mp - (full - 20)
    };
    assert_eq!((guarded(true), guarded(false)), (3, 0));
}

#[test]
fn critical_counter_marks_a_block_within_0_1333_s_as_a_just_guard() {
    // CRITICAL_COUNTER (104; II.1 "Counter window and just guard"): Guard Begin arms the
    // just-guard timer (0.1333 s) when the ability is equipped; a hit guarded while it runs opens
    // the counter window with the critical flag (controller+0x878 bit 8); later, or without the
    // ability, the window opens without it.
    let guard_then_hit = |equipped: bool, wait: usize| {
        let mut bench = Bench::new();
        let z = bench.last().position.z;
        bench.foe = Some(Foe::at(Vec3::new(150.0, 0.0, z), 180.0, 40.0));
        if equipped {
            bench.sora.status_mut().abilities.set(Ability::CriticalCounter, 1, 1);
        }
        bench.step(press(PadButton::GUARD, Vec2::ZERO));
        bench.run(wait);
        let hit = foe_hit(&bench, Kind::Small, 0);
        bench.sora.take_hit(hit);
        bench.step(idle());
        let a = bench.sora.actions();
        (a.critical, a.counter_window > 0.9)
    };
    assert_eq!(guard_then_hit(true, 4), (true, true), "0.08 s after the guard began");
    assert_eq!(guard_then_hit(true, 12), (false, true), "0.2 s: too late for a just guard");
    assert_eq!(guard_then_hit(false, 4), (false, true), "without CRITICAL_COUNTER");
}
