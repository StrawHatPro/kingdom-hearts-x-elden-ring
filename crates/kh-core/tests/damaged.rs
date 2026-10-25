//! Sora being hit: combat strand E (build/sora_tmp/combat/P2_E_section.md: the victim side of
//! TakeDamage 8.4, the reaction gate 9.1, the Damage state 9.2 - 9.5, HP rules 9.7, death 9.8).
//! The numbers asserted are the strand's derived numbers at 60 Hz (its "Numbers [D]" paragraphs).

use kh_core::enums::ETresDamageKind as Kind;
use kh_core::host::EventKind;
use kh_core::states::{direction_index, DamageHandler, HitOnPlayer};
use kh_core::status::LoadoutValue;
use kh_core::test_world::TestWorld;
use kh_core::{AnimId, Input, Loadout, Output, PadButton, PadButtons, PadState, Pose, Sora, StateId, Status, TargetId, Tuning, Vec2, Vec3, SORA_PARAMS};

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

fn level99(equipped: Option<&[&str]>) -> Status {
    let path = format!("{}/../../sheets/port/sora-loadout-99.json", env!("CARGO_MANIFEST_DIR"));
    let json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let mut loadout = Loadout::new("sora-loadout-99");
    for row in json["rows"].as_array().unwrap() {
        let key = row[0].as_str().unwrap();
        let value = match (key, equipped) {
            ("abilities.equipped", Some(list)) => LoadoutValue::List(list.iter().map(|s| s.to_string()).collect()),
            _ => cell(&row[1]).unwrap(),
        };
        loadout.set(key, &value).unwrap();
    }
    loadout.resolve().unwrap();
    Status::from_loadout(&loadout)
}

struct Bench {
    sora: Sora,
    world: TestWorld,
    outs: Vec<Output>,
}

impl Bench {
    fn new(status: Status) -> Bench {
        let origin = Vec3::new(0.0, 0.0, Sora::origin_height_on_floor(&SORA_PARAMS));
        let sora = Sora::new(SORA_PARAMS, Pose { position: origin, yaw: 0.0 }).with_tuning(Tuning::GAME_EXACT).with_status(status);
        let mut bench = Bench { sora, world: TestWorld::floor(), outs: Vec::new() };
        bench.run(5);
        bench
    }

    fn step(&mut self, input: Input) -> Output {
        let out = self.sora.step(DT, &input, &self.world);
        self.outs.push(out.clone());
        out
    }

    fn run(&mut self, steps: usize) {
        for _ in 0..steps {
            self.step(Input::default());
        }
    }

    /// Steps until the state is `state` (at most `limit` steps); returns the steps taken.
    fn until(&mut self, state: StateId, limit: usize) -> usize {
        for k in 1..=limit {
            if self.step(Input::default()).state == state {
                return k;
            }
        }
        panic!("the state did not become {state:?} within {limit} steps (it is {:?})", self.outs.last().unwrap().state);
    }

    fn last(&self) -> &Output {
        self.outs.last().unwrap()
    }
}

/// A hit from an attacker standing 150 cm in FRONT of him (he faces +X), pushing him back (-X).
fn hit(kind: Kind, amount: f32) -> HitOnPlayer {
    HitOnPlayer { direction: Vec3::new(-1.0, 0.0, 0.0), causer_yaw: 180.0, ..HitOnPlayer::new(kind, amount, TargetId(1), Vec3::new(150.0, 0.0, 0.0)) }
}

#[test]
fn the_direction_index_of_e_9_3() {
    // Attacker in front (the hit pushes him backward): 0; behind: 1; to his right: 2; left: 3.
    assert_eq!(direction_index(Vec3::new(-1.0, 0.0, 0.0), 0.0, false), 0);
    assert_eq!(direction_index(Vec3::new(1.0, 0.0, 0.0), 0.0, false), 1);
    assert_eq!(direction_index(Vec3::new(0.0, -1.0, 0.0), 0.0, false), 2);
    assert_eq!(direction_index(Vec3::new(0.0, 1.0, 0.0), 0.0, false), 3);
    // The two-way variant; no direction at all: 0.
    assert_eq!((direction_index(Vec3::new(0.0, -1.0, 0.0), 0.0, true), direction_index(Vec3::new(1.0, 0.1, 0.0), 0.0, true)), (0, 1));
    assert_eq!(direction_index(Vec3::ZERO, 77.0, false), 0);
}

#[test]
fn a_small_hit_on_the_ground_slides_91_7_cm_and_ends_with_its_clip() {
    // E 9.4 Small: 550 x scale cm/s, x 0.9 per frame -> 91.7 cm of slide; clip 0201 (1.1667 s,
    // TresEnableChange at 0.4667 s); untouched the state ends at the clip's end.
    let mut bench = Bench::new(level99(None));
    let hp = bench.sora.status().hp;
    let start = bench.last().position;
    bench.sora.take_hit(hit(Kind::Small, 30.0));
    let out = bench.step(Input::default());
    assert_eq!((out.state, out.anim.name), (StateId::Damage, AnimId::A0201));
    assert_eq!(bench.sora.status().hp, hp - 30);
    let hit_event = out.events.iter().find_map(|e| if let EventKind::PlayerHit { hp_lost, hp_after, .. } = e.kind { Some((hp_lost, hp_after)) } else { None });
    assert_eq!(hit_event, Some((30, hp - 30)));
    let view = bench.sora.damage_reaction().unwrap();
    assert_eq!((view.handler, view.index, view.hits), (DamageHandler::Small, 0, 1));
    // The first movement tick moves him at 550 cm/s, each later one at 0.9 of the one before.
    let first = bench.step(Input::default());
    assert!((first.displacement_velocity.x + 550.0).abs() < 0.05, "first tick {:?}", first.displacement_velocity);
    let second = bench.step(Input::default());
    assert!((second.displacement_velocity.x + 550.0 * 0.9).abs() < 0.05);
    let steps = 2 + bench.until(StateId::Idle, 100);
    let slide = start.x - bench.last().position.x;
    assert!((slide - 91.7).abs() < 1.5, "slid {slide} cm");
    assert!((steps as f32 * DT - 1.1667).abs() <= 2.0 * DT, "the reaction lasted {} s", steps as f32 * DT);
    assert!(!bench.sora.status().in_damage_state);
}

#[test]
fn a_held_stick_ends_a_small_reaction_at_the_change_point_and_attack_is_allowed_there() {
    // E 9.3 "common exits", 9.5: from TresEnableChange (0.4667 s) a held stick ends the state;
    // CanAttack: kind in {1, 2, 7, 8, 9, 10} with m_bEnableChange.
    let mut bench = Bench::new(level99(None));
    bench.sora.take_hit(hit(Kind::Small, 10.0));
    let mut steps = 0;
    loop {
        let out = bench.step(Input { move_stick: Vec2::new(0.0, 1.0), ..Input::default() });
        steps += 1;
        if out.state != StateId::Damage {
            break;
        }
        assert!(steps < 100);
    }
    assert!((steps as f32 * DT - 0.4667).abs() <= 3.0 * DT, "ended after {} s", steps as f32 * DT);
    // An attack press before the change point is refused, after it accepted.
    let mut bench = Bench::new(level99(None));
    bench.sora.take_hit(hit(Kind::Small, 10.0));
    let press = Input::default().with_pad(PadState::from_held(PadButtons::NONE.with(PadButton::ATTACK), PadButtons::NONE));
    bench.step(Input::default());
    for _ in 0..10 {
        assert_eq!(bench.step(press).state, StateId::Damage);
    }
    bench.run(20);
    // (The request is made in the UI phase of the press frame: the attack begins in the next step.)
    assert_eq!(bench.step(press).state, StateId::Damage);
    assert_eq!(bench.step(Input::default()).state, StateId::Attack, "0.5 s after the hit an attack starts out of the reaction");
}

#[test]
fn consecutive_hits_re_enter_the_state_and_alternate_the_clip() {
    // E 9.1: a hit during DAMAGE re-enters DAMAGE (no invincibility); the counter is carried;
    // every second consecutive hit plays 0205 / 0206 instead of 0201 / 0202.
    let mut bench = Bench::new(level99(None));
    bench.sora.take_hit(hit(Kind::Small, 5.0));
    bench.run(10);
    bench.sora.take_hit(hit(Kind::Small, 5.0));
    let out = bench.step(Input::default());
    assert_eq!((out.state, out.anim.name, bench.sora.damage_reaction().unwrap().hits), (StateId::Damage, AnimId::A0205, 2));
    bench.run(10);
    bench.sora.take_hit(hit(Kind::Small, 5.0));
    let out = bench.step(Input::default());
    assert_eq!((out.anim.name, bench.sora.damage_reaction().unwrap().hits), (AnimId::A0201, 3));
    assert!(bench.sora.status().in_damage_state);
}

#[test]
fn a_blow_throws_him_at_2000_with_the_brake_and_he_gets_up() {
    // E 9.4 Blow: 2000 cm/s horizontally x 0.95 per frame (asymptotic 667 cm); gravity off and
    // Z from 300 x 0.95 per frame until the horizontal speed is <= 700 (21 frames), then gravity;
    // recoverable from 0.2 s after the hit; start clip 0211 (attacker in front), then the loop
    // 0212, then down / get-up 0213.
    let mut bench = Bench::new(level99(None));
    let start = bench.last().position;
    bench.sora.take_hit(hit(Kind::Blow, 20.0));
    let out = bench.step(Input::default());
    assert_eq!((out.state, out.anim.name), (StateId::Damage, AnimId::A0211));
    let first = bench.step(Input::default());
    assert!((first.displacement_velocity.x + 2000.0).abs() < 0.2 && (first.displacement_velocity.z - 300.0).abs() < 0.2, "launch {:?}", first.displacement_velocity);
    assert!(!first.on_floor);
    let mut names = vec![AnimId::A0211];
    let mut recoverable_at = None;
    let mut peak = 0.0f32;
    for k in 0..400 {
        let out = bench.step(Input::default());
        if names.last() != Some(&out.anim.name) {
            names.push(out.anim.name);
        }
        peak = peak.max(out.position.z);
        if recoverable_at.is_none() && bench.sora.damage_reaction().is_some_and(|v| v.recoverable) {
            recoverable_at = Some((k + 2) as f32 * DT);
        }
        if out.state == StateId::Idle {
            break;
        }
    }
    assert_eq!(bench.last().state, StateId::Idle);
    // THE CODE (decompiled C of 0x14135B300, case 3): while the start clip plays the case breaks
    // before the timer test (the strand's pseudocode left that break out): the start clip (0.1 s)
    // plays out, the loop 0212 starts, and only then the 0.6 s of flight begin.
    assert_eq!(names[..3], [AnimId::A0211, AnimId::A0212, AnimId::A0213], "{names:?}");
    let distance = start.x - bench.last().position.x;
    assert!(distance > 600.0 && distance < 680.0, "thrown {distance} cm (asymptote 667)");
    // (The spec derives +66 cm from a Z brake that starts at once; with the start clip waited out
    // in sub 3 - where only the horizontal brake runs - the Z speed stays 300 for 0.1 s longer.)
    assert!(peak > 80.0 && peak < 100.0, "rose {peak} cm");
    let at = recoverable_at.expect("recoverable");
    assert!((0.2..0.3).contains(&at), "recoverable from {at} s");
}

#[test]
fn toss_and_launch_throw_straight_up() {
    // E 9.4 Toss / Launch: no horizontal velocity; 1500 / 2500 x scale up under GravityScale 3
    // (2940 cm/s^2): apex 383 cm / 1063 cm; Toss lands and plays 0213; Launch's 1.2 s fall timer
    // ends before the ground: the state finishes in the air.
    let mut bench = Bench::new(level99(None));
    bench.sora.take_hit(hit(Kind::Toss, 10.0));
    let mut peak = 0.0f32;
    let mut got_up = false;
    for _ in 0..400 {
        let out = bench.step(Input::default());
        peak = peak.max(out.position.z);
        got_up |= out.anim.name == AnimId::A0213 && out.state == StateId::Damage;
        assert!(out.position.x.abs() < 1.0);
        if out.state == StateId::Idle {
            break;
        }
    }
    assert!((peak - 383.0).abs() < 12.0, "Toss apex {peak}");
    assert!(got_up && bench.last().state == StateId::Idle);
    let mut bench = Bench::new(level99(None));
    bench.sora.take_hit(hit(Kind::Launch, 10.0));
    let (mut peak, mut ended_at) = (0.0f32, None);
    for _ in 0..400 {
        let out = bench.step(Input::default());
        peak = peak.max(out.position.z);
        if out.state != StateId::Damage {
            ended_at = Some(out.position.z);
            break;
        }
    }
    assert!((peak - 1063.0).abs() < 25.0, "Launch apex {peak}");
    assert!(ended_at.is_some_and(|z| z > 50.0), "the Launch reaction ended in the air at {ended_at:?}");
}

#[test]
fn a_parabola_is_a_45_degree_throw_and_a_beat_slams_and_bounces() {
    // E 9.4 Parabola: 1500 x scale at 45 degrees (1061 horizontally, 1061 up) under normal gravity.
    let mut bench = Bench::new(level99(None));
    bench.sora.take_hit(hit(Kind::Parabola, 10.0));
    bench.step(Input::default());
    let first = bench.step(Input::default());
    assert!((first.displacement_velocity.x + 1060.66).abs() < 0.5 && first.displacement_velocity.z > 1000.0, "parabola launch {:?}", first.displacement_velocity);
    bench.until(StateId::Idle, 600);
    // Beat from the air: slammed down at 2500, bounces (rebound 1000 under GravityScale 3: 170 cm)
    // and the state ends at the top of the bounce.
    let mut bench = Bench::new(level99(None));
    bench.step(Input { jump_pressed: true, jump_held: true, ..Input::default() });
    for _ in 0..20 {
        bench.step(Input { jump_held: true, ..Input::default() });
    }
    assert!(bench.last().position.z > 100.0);
    bench.sora.take_hit(hit(Kind::Beat, 10.0));
    bench.step(Input::default());
    let down = bench.step(Input::default());
    assert!(down.displacement_velocity.z <= -2400.0 || down.on_floor, "beat {:?}", down.displacement_velocity);
    let (mut landed, mut bounce) = (false, 0.0f32);
    for _ in 0..300 {
        let out = bench.step(Input::default());
        if out.on_floor {
            landed = true;
        }
        if landed {
            bounce = bounce.max(out.position.z);
        }
        if out.state != StateId::Damage {
            break;
        }
    }
    assert!(landed && (bounce - 170.0).abs() < 15.0, "bounce height {bounce}");
}

#[test]
fn second_chance_keeps_one_hp_and_the_next_hit_kills() {
    // E 9.7 (the rule itself is the status implementer's): Second Chance keeps 1 HP on a hit
    // taken at 2 HP or more; a fresh hit at 1 HP outside the Withstand Combo window kills.
    // E 9.8: the Die state (59), thrown away from the causer at 2000 cm/s, Z 125; PlayerDied.
    let mut bench = Bench::new(level99(Some(&["LAST_LEAVE"])));
    bench.sora.take_hit(hit(Kind::Small, 100_000.0));
    let out = bench.step(Input::default());
    assert_eq!((bench.sora.status().hp, out.state), (1, StateId::Damage));
    bench.until(StateId::Idle, 200);
    bench.run(60);
    bench.sora.take_hit(hit(Kind::Small, 5.0));
    let out = bench.step(Input::default());
    assert_eq!((bench.sora.status().hp, out.state), (0, StateId::Die));
    assert!(out.events.iter().any(|e| matches!(e.kind, EventKind::PlayerDied { causer: TargetId(1) })));
    let thrown = bench.step(Input::default());
    assert!((thrown.displacement_velocity.x + 2000.0).abs() < 0.2 && thrown.displacement_velocity.z > 100.0, "die throw {:?}", thrown.displacement_velocity);
    // He stays dead: no input moves him, further hits do nothing.
    for _ in 0..300 {
        bench.step(Input { move_stick: Vec2::new(0.0, 1.0), jump_pressed: true, ..Input::default() });
    }
    assert_eq!(bench.last().state, StateId::Die);
    bench.sora.take_hit(hit(Kind::Blow, 5.0));
    bench.run(5);
    assert_eq!(bench.last().state, StateId::Die);
}

#[test]
fn guarded_and_no_reaction_types() {
    // E 8.4: reaction type 1 (NO_REACTION): damage without a reaction; types 3 - 6: no damage.
    let mut bench = Bench::new(level99(None));
    let hp = bench.sora.status().hp;
    bench.sora.take_hit(HitOnPlayer { reaction_type: 1, ..hit(Kind::Blow, 40.0) });
    assert_eq!((bench.step(Input::default()).state, bench.sora.status().hp), (StateId::Idle, hp - 40));
    // Result 5 (GUARD) outside a guard: `OnGuardedHit`'s gate refuses: nothing happens.
    bench.sora.take_hit(HitOnPlayer { reaction_type: 5, ..hit(Kind::Blow, 40.0) });
    assert_eq!((bench.step(Input::default()).state, bench.sora.status().hp), (StateId::Idle, hp - 40));
    // Result 3 (REFLECT; spec 8.4: iface +0x448): no HP loss, the weapon recoil (state 66):
    // AttackReflect 0.9667 s, pushed back at 400 x 0.95 per frame (about 120 cm in all).
    let x = bench.last().position.x;
    bench.sora.take_hit(HitOnPlayer { reaction_type: 3, ..hit(Kind::Blow, 40.0) });
    assert_eq!((bench.step(Input::default()).state, bench.sora.status().hp), (StateId::Reflect, hp - 40));
    assert_eq!(bench.last().anim.name, AnimId::from_name("AttackReflect").unwrap());
    let mut steps = 1;
    while bench.step(Input::default()).state == StateId::Reflect {
        steps += 1;
    }
    assert!((57..=60).contains(&steps), "the recoil lasted {steps} steps (0.9667 s)");
    let back = x - bench.last().position.x;
    assert!(back.abs() > 100.0 && back.abs() < 130.0, "pushed {back} cm");
}
