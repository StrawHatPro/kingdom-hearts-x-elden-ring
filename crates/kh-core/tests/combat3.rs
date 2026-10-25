//! Combat, third pass (2026-10-12): the bad statuses on Sora (docs/sora-combat-spec.md 9.9), the
//! C-level remainders of the second pass (map-hit recoil, the dive's shockwave, the guard
//! reaction of a GUARD_REFLECT result) and the combat abilities that the audit
//! (build/sora_tmp/impl_combat/ABILITY_AUDIT.md) found missing. One test per feature added.

use kh_core::combat::badstat::kind as bad;
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
    fn in_world(status: Status, dummies: &[TargetInfo], world: TestWorld) -> Bench {
        let origin = Vec3::new(0.0, 0.0, Sora::origin_height_on_floor(&SORA_PARAMS));
        let sora = Sora::new(SORA_PARAMS, Pose { position: origin, yaw: 0.0 }).with_tuning(Tuning::GAME_EXACT).with_status(status);
        let mut host = RecordingHost::new();
        host.targets.extend_from_slice(dummies);
        let mut bench = Bench { sora, host, world, outs: Vec::new() };
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

    fn events(&self) -> impl Iterator<Item = &EventKind> {
        self.outs.iter().flat_map(|o| o.events.iter().map(|e| &e.kind))
    }

    fn effects(&self) -> Vec<(u32, &'static str)> {
        self.events().filter_map(|e| if let EventKind::EffectSpawn { spawn, .. } = e { Some((spawn.id.0, spawn.name)) } else { None }).collect()
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
}

/// A hit on Sora from 2 m in front of him, carrying a bad status.
fn status_hit(kind: Kind, amount: f32, status: &'static str, time: f32) -> HitOnPlayer {
    HitOnPlayer { direction: Vec3::new(-1.0, 0.0, 0.0), causer_yaw: 180.0, bad_status: status, bad_time: time, ..HitOnPlayer::new(kind, amount, TargetId(1), Vec3::new(200.0, 0.0, 0.0)) }
}

// ------------------------------------------------------------------------------------ 9.9 bad statuses

/// Spec 9.9.2: a stunning hit flinches him first; standing on the ground the stun state (122)
/// follows: start clip, loop for as long as the status lasts (its time counts only in the
/// state), end clip, Idle. The stun effect row 0xC58 is attached and removed.
#[test]
fn a_stun_holds_him_in_state_122_for_its_time() {
    let mut bench = Bench::new(level99(None), &[dummy(1, 200.0, 0.0, 0.0)]);
    bench.sora.take_hit(status_hit(Kind::Small, 10.0, "STUN", 2.0));
    bench.step(&[], &[]);
    assert_eq!(bench.sora.combat().bad.kind, bad::STUN);
    let entered = bench.until(120, "the stun state", |_, o| o.state == StateId::BadstatStun);
    let from = bench.outs.len();
    // An attack press does nothing while he is stunned.
    for _ in 0..30 {
        bench.tap(PadButton::ATTACK);
        bench.step(&[], &[]);
    }
    assert_eq!(bench.outs.last().unwrap().state, StateId::BadstatStun, "no attack out of a stun");
    let lasted = 60 + bench.until(400, "the stun's end", |b, _| b.sora.combat().bad.kind == 0);
    // 2.0 s counted from the first tick of the state (the flinch before it does not count).
    assert!((118..=124).contains(&lasted), "the stun lasted {lasted} steps in the state (entered after {entered})");
    bench.until(200, "Idle after the stun", |_, o| o.state == StateId::Idle);
    assert_eq!(bench.clips(from - 1)[..3], ["BadStatStunStart", "BadStatStunLoop", "BadStatStunEnd"]);
    assert!(bench.effects().contains(&(0xC58, "vs_abn_stn1")));
    assert!(bench.events().any(|e| matches!(e, EventKind::BadStatus { from: 12, to: 0, .. })));
}

/// Spec 9.9.2: every stick flick (the stick leaving the centre, or swinging by 90 degrees or
/// more) takes 0.3 s off the stun.
#[test]
fn stick_flicks_shorten_a_stun() {
    let mut bench = Bench::new(level99(None), &[dummy(1, 200.0, 0.0, 0.0)]);
    bench.sora.take_hit(status_hit(Kind::Small, 10.0, "STUN", 6.0));
    bench.until(200, "the stun state", |_, o| o.state == StateId::BadstatStun);
    // Wait for the loop, then flick left / right every other step: 10 flicks = 3.0 s.
    bench.until(200, "the stun loop", |_, o| o.anim.name.name() == "BadStatStunLoop");
    let before = bench.sora.combat().bad.time;
    for k in 0..10 {
        let x = if k % 2 == 0 { 1.0 } else { -1.0 };
        bench.step_with(&[], &[], Vec2::new(x, 0.0));
    }
    bench.step(&[], &[]);
    let after = bench.sora.combat().bad.time;
    let taken = before - after;
    assert!((taken - (3.0 + 11.0 * DT)).abs() < 0.05, "10 flicks took {taken} s off the stun");
    assert_eq!(bench.outs.last().unwrap().state, StateId::BadstatStun);
}

/// Spec 9.9.1 (0x1405A2350): any damaging hit cures a stun.
#[test]
fn a_damaging_hit_cures_a_stun() {
    let mut bench = Bench::new(level99(None), &[dummy(1, 200.0, 0.0, 0.0)]);
    bench.sora.take_hit(status_hit(Kind::Small, 10.0, "STUN", 8.0));
    bench.until(200, "the stun state", |_, o| o.state == StateId::BadstatStun);
    bench.idle(30);
    bench.sora.take_hit(status_hit(Kind::Small, 10.0, "NONE", 0.0));
    bench.step(&[], &[]);
    assert_eq!(bench.sora.combat().bad.kind, 0, "the hit cured the stun");
    bench.until(300, "Idle", |_, o| o.state == StateId::Idle);
    assert!(!bench.states()[3..].contains(&StateId::BadstatStun), "no second stun state: {:?}", bench.states());
}

/// Spec 9.9.3: FREEZE with a time starts state 123 at the gate (no Damage state): the pose stands
/// still, he does not move, the ice effect covers him; when the time is up the ice breaks.
#[test]
fn a_freeze_stops_him_and_his_pose_for_its_time() {
    let mut bench = Bench::new(level99(None), &[dummy(1, 200.0, 0.0, 0.0)]);
    bench.idle(20);
    bench.sora.take_hit(status_hit(Kind::Small, 10.0, "FREEZE", 1.5));
    bench.step(&[], &[]);
    let out = bench.step(&[], &[]);
    assert_eq!(out.state, StateId::BadstatFreeze);
    assert!(bench.sora.combat().bad.frozen && bench.sora.combat().bad.kind == bad::FREEZE);
    let (clip, time, at) = (out.anim.name, out.anim.time, out.position);
    let mid = bench.idle(40);
    assert_eq!((mid.anim.name, mid.anim.time), (clip, time), "the pose is paused");
    assert!((mid.position - at).length() < 0.5);
    // A second freezing hit only rewrites the time.
    bench.sora.take_hit(status_hit(Kind::Small, 10.0, "FREEZE", 1.0));
    let lasted = bench.until(200, "the thaw", |_, o| o.state != StateId::BadstatFreeze);
    assert!((58..=63).contains(&lasted), "the refreshed freeze lasted {lasted} steps");
    assert_eq!(bench.sora.combat().bad.kind, 0);
    let effects = bench.effects();
    assert!(effects.contains(&(0xC1C, "vs_abn_frz0")) && effects.contains(&(0xC1F, "vs_abn_frz_out0")), "ice and ice-break effects: {effects:?}");
    assert!(!bench.states().contains(&StateId::Damage), "a freeze is not a flinch");
}

/// Spec 9.9.4: BURN on a hit of damage kind NONE starts state 124: he hops about (start clip ->
/// launch -> loop in the air -> landing -> a new direction), loses Ceil(5 % max HP) at every hop
/// after the first, hits do not flinch him, and when the time (counted in the state) is up he
/// lands, recovers and is back in Idle with his speeds restored.
#[test]
fn a_burn_makes_him_hop_about_and_costs_hp_per_hop() {
    let mut bench = Bench::new(level99(None), &[dummy(1, 200.0, 0.0, 0.0)]);
    let (hp_max, start) = (bench.sora.status().hp_max, bench.outs.last().unwrap().position);
    bench.sora.take_hit(status_hit(Kind::None, 0.0, "BURN", 4.0));
    bench.step(&[], &[]);
    let out = bench.step(&[], &[]);
    assert_eq!(out.state, StateId::BadstatBurn);
    let hp_before = bench.sora.status().hp;
    let from = bench.outs.len();
    // A small hit in the middle: HP goes, the state stays.
    bench.idle(60);
    bench.sora.take_hit(status_hit(Kind::Small, 7.0, "NONE", 0.0));
    bench.idle(2);
    assert_eq!(bench.outs.last().unwrap().state, StateId::BadstatBurn, "no flinch while burning");
    bench.until(900, "Idle after the burn", |_, o| o.state == StateId::Idle);
    let burn: Vec<&Output> = bench.outs[from..].iter().filter(|o| o.state == StateId::BadstatBurn).collect();
    let starts = bench.outs[from - 1..].windows(2).filter(|w| w[1].anim.name.name() == "BadStatBurnStart" && w[1].anim.play_id != w[0].anim.play_id).count() + 1;
    let apex = burn.iter().map(|o| o.position.z - start.z).fold(0.0f32, f32::max);
    assert!(starts >= 3, "{starts} hops");
    assert!((185.0..=215.0).contains(&apex), "a hop rises {apex} cm (m_BadStatBurnJumpHeight 200)");
    let travelled = burn.iter().map(|o| (o.position - start).truncate().length()).fold(0.0f32, f32::max);
    assert!(travelled > 60.0, "he moves while hopping: {travelled} cm");
    let per_hop = (0.05 * hp_max as f32).ceil() as i32;
    let lost = hp_before - bench.sora.status().hp - 7;
    assert_eq!(lost, per_hop * (starts as i32 - 1), "{starts} hops, {per_hop} HP per hop after the first");
    assert!(bench.clips(from).contains(&"BadStatBurnEnd"));
    assert!(bench.effects().contains(&(0xC08, "vs_abn_brn0")));
    assert_eq!(bench.sora.combat().bad.kind, 0);
}

/// Spec 9.9.5 (rewritten by the fourth pass, 2026-10-12): with SNEEZE on him the combo runs as
/// usual until `PlayAttack` of the step `combo >= MaxCombo - 2` latches controller+0x920; the
/// NEXT attack - the one that would be the finisher - is the sneeze: attack kind 0x1E, clip
/// `BadStatSneeze`, counted as the combo's last finisher (no follow-up). The latch goes when the
/// chain ends; after the status's time the combo has its finisher again.
#[test]
fn a_sneeze_replaces_the_finisher() {
    let combo = |sneezing: bool| {
        let mut bench = Bench::new(level99(Some(&["COMBO_MASTER"])), &[dummy(1, 150.0, 0.0, 0.0)]);
        if sneezing {
            bench.sora.take_hit(HitOnPlayer { kind: Kind::Without, ..status_hit(Kind::Small, 1.0, "SNEEZE", 30.0) });
            bench.step(&[], &[]);
            assert_eq!(bench.sora.combat().bad.kind, bad::SNEEZE);
        }
        let mut latched_at = None;
        for _ in 0..400 {
            let out = bench.tap(PadButton::ATTACK);
            bench.step(&[], &[]);
            if latched_at.is_none() && bench.sora.combat().sneeze_next {
                latched_at = Some(bench.sora.combat().history.len());
            }
            let _ = out;
            if bench.sora.combat().history.len() >= 4 {
                break;
            }
        }
        bench.until(300, "Idle after the combo", |_, o| o.state == StateId::Idle);
        (bench.sora.combat().history.clone(), latched_at, bench.clips(0).contains(&"BadStatSneeze"), bench.sora.combat().sneeze_next)
    };
    // MaxCombo 4 without Combo Plus: steps 0, 1, 2 are normal, step 3 is the finisher (kind 4).
    let (plain, latched, sneezed, _) = combo(false);
    assert_eq!((plain.len(), plain[3], latched, sneezed), (4, 4, None, false), "{plain:x?}");
    // Sneezing: the third attack (combo 2 = MaxCombo - 2) latches, the fourth is the sneeze.
    let (kinds, latched, sneezed, still) = combo(true);
    assert_eq!(kinds.len(), 4, "{kinds:x?}");
    assert_eq!((&kinds[..3], kinds[3]), (&plain[..3], 0x1E), "{kinds:x?}");
    assert_eq!(latched, Some(3), "the latch rises when the third attack is played");
    assert!(sneezed && !still, "the sneeze clip played; the latch is cleared at the chain's end");
}

/// Spec 9.9.1: STOP runs his own time at 0.1 for its (real) time; HONEY slows him by the row's
/// parameter per application down to 0.1; both give his time back when they end.
#[test]
fn stop_and_honey_slow_his_own_time() {
    let mut bench = Bench::new(level99(None), &[dummy(1, 200.0, 0.0, 0.0)]);
    bench.sora.take_hit(HitOnPlayer { kind: Kind::Without, ..status_hit(Kind::Small, 1.0, "STOP", 1.0) });
    bench.step(&[], &[]);
    assert_eq!((bench.sora.combat().bad.kind, bench.sora.combat().bad.dilation), (bad::STOP, 0.1));
    // Half a second of running at a tenth of his time: about a twentieth of the distance.
    let at = bench.outs.last().unwrap().position;
    for _ in 0..30 {
        bench.step_with(&[], &[], Vec2::new(0.0, 1.0));
    }
    let slow = (bench.outs.last().unwrap().position - at).length();
    assert!(slow < 30.0, "stopped, he ran {slow} cm in 0.5 s");
    let lasted = 31 + bench.until(100, "the stop's end", |b, _| b.sora.combat().bad.kind == 0);
    assert!((59..=62).contains(&lasted), "STOP lasted {lasted} real steps");
    assert_eq!(bench.sora.combat().bad.dilation, 1.0);
    // Honey twice: 1.0 - 0.3 - 0.3.
    for _ in 0..2 {
        bench.sora.take_hit(HitOnPlayer { kind: Kind::Without, bad_param: 0.3, ..status_hit(Kind::Small, 1.0, "HONEY", 2.0) });
        bench.step(&[], &[]);
    }
    assert!((bench.sora.combat().bad.dilation - 0.4).abs() < 1.0e-5);
    bench.until(200, "the honey's end", |b, _| b.sora.combat().bad.kind == 0);
    assert_eq!(bench.sora.combat().bad.dilation, 1.0);
}

/// Spec 9.9.1: HP_BREAK caps his max HP at the HP he had when it landed, for its time.
#[test]
fn hp_break_caps_max_hp_for_its_time() {
    let mut bench = Bench::new(level99(None), &[dummy(1, 200.0, 0.0, 0.0)]);
    let full = bench.sora.status().hp_max;
    bench.sora.take_hit(HitOnPlayer { kind: Kind::Without, ..status_hit(Kind::Small, 40.0, "HP_BREAK", 1.0) });
    bench.idle(3);
    let capped = bench.sora.status().hp_max;
    assert!(capped < full && capped == bench.sora.status().hp, "max HP {full} -> {capped}");
    bench.until(100, "the break's end", |b, _| b.sora.combat().bad.kind == 0);
    bench.idle(2);
    assert_eq!(bench.sora.status().hp_max, full);
}

/// Spec 9.9.1: the `*_GUARD` abilities refuse their status (`Immune`, pawn vt+0xF50).
#[test]
fn stun_guard_refuses_the_stun() {
    // (Stun Protection comes with equipment - Keyblade WEP_KEYBLADE_SO_08, two accessories -: worn, not owned.)
    let mut status = level99(None);
    status.abilities.add_worn(kh_core::enums::ETresAbilityKind::StunGuard);
    let mut bench = Bench::new(status, &[dummy(1, 200.0, 0.0, 0.0)]);
    bench.sora.take_hit(status_hit(Kind::Small, 10.0, "STUN", 5.0));
    bench.until(300, "Idle", |_, o| o.state == StateId::Idle);
    assert_eq!(bench.sora.combat().bad.applied, 0);
    assert!(!bench.states().contains(&StateId::BadstatStun));
}

// ------------------------------------------------------------------------------------ item 3: C-level remainders

/// Spec 8.2.5 step 9 / 8.5.1 (Attack slots 24 and 140): the first swing of a combo without a
/// target that meets a wall while the clip's map-hit-reflect window is open is repelled by it:
/// the weapon recoil (state 66) with no causer. One wall spark only.
#[test]
fn a_first_swing_into_a_wall_is_repelled_during_the_reflect_window() {
    let wall = TestWorld::floor().with_plane(Vec3::NEG_X, -150.0);
    let mut bench = Bench::in_world(level99(Some(&[])), &[], wall);
    bench.tap(PadButton::ATTACK);
    bench.until(60, "the recoil", |_, o| o.state == StateId::Reflect);
    let combat = bench.sora.combat();
    assert_eq!((combat.wall_hits, combat.map_recoils), (1, 1));
    assert_eq!(bench.sora.recoil().map(|r| r.causer), Some(TargetId::NONE));
    assert_eq!(bench.effects().iter().filter(|e| e.1 == "VW_SO010_WHT0").count(), 1);
    bench.until(120, "Idle after the recoil", |_, o| o.state == StateId::Idle);
    // With a target the same swing is not repelled by the level (the recoil needs "no target").
    let wall = TestWorld::floor().with_plane(Vec3::NEG_X, -150.0);
    let mut bench = Bench::in_world(level99(Some(&[])), &[dummy(1, 0.0, 300.0, 0.0)], wall);
    bench.tap(PadButton::ATTACK);
    bench.idle(60);
    assert_eq!(bench.sora.combat().map_recoils, 0);
}

/// The geometry of Attack slot 140 (0x14133D4D0): in front within 80 degrees of yaw and pitch,
/// measured from 50 cm above his root; a ceiling above 40 degrees of pitch does not repel.
#[test]
fn the_map_recoil_cone() {
    use kh_core::combat::hit::map_recoil;
    let (root, wall) = (Vec3::ZERO, Vec3::NEG_X);
    assert!(map_recoil(root, 0.0, Vec3::new(100.0, 0.0, 80.0), wall));
    assert!(!map_recoil(root, 0.0, Vec3::new(-100.0, 0.0, 80.0), wall), "behind him");
    assert!(!map_recoil(root, 0.0, Vec3::new(10.0, 100.0, 80.0), wall), "more than 80 degrees to the side");
    assert!(map_recoil(root, 90.0, Vec3::new(10.0, 100.0, 80.0), wall), "the same point when he faces it");
    assert!(map_recoil(root, 0.0, Vec3::new(100.0, 0.0, 120.0), Vec3::NEG_Z), "below 40 degrees of pitch any surface repels");
    assert!(map_recoil(root, 0.0, Vec3::new(100.0, 0.0, 190.0), wall), "above 40 degrees a wall still repels");
    assert!(!map_recoil(root, 0.0, Vec3::new(100.0, 0.0, 190.0), Vec3::NEG_Z), "above 40 degrees a ceiling does not");
}

/// Jumps and, at the top, starts an attack of forced kind `kind` on the dummy below.
fn air_down(kind: u8) -> Bench {
    let mut bench = Bench::new(level99(Some(&["HIGHJUMP", "AIR_DOWN", "SONIC_DOWN"])), &[dummy(1, 160.0, 0.0, 0.0)]);
    for n in 0..200 {
        let out = bench.step(&[PadButton::JUMP], if n == 0 { &[] } else { &[PadButton::JUMP] });
        if n > 10 && out.velocity.z <= 0.0 && !out.on_floor {
            break;
        }
    }
    assert!(bench.outs.last().unwrap().position.z > 300.0, "he is high above the dummy");
    bench.sora.combat_mut().guard.offer = Some(kh_core::combat::guard::CounterOffer { source: StateId::Jump, kind, target: TargetId(1), just_guard: false, push: false });
    bench.tap(PadButton::ATTACK);
    bench.until(10, "the attack state", |_, o| o.state == StateId::Attack);
    assert_eq!(bench.sora.attack().unwrap().kind, kind);
    bench.until(400, "Idle after the dive", |_, o| o.state == StateId::Idle);
    bench
}

/// Spec 8.8: the finisher air-down (kind 0x0E, clip 1318) spawns the shot DOWNIMPACT once it has
/// landed AND its Shot notify (0.6667 s) has fired: a stationary sphere growing from 50 to 200 cm
/// for 0.5 s that hits with row `KB_DOWN_BOMB`. The non-finisher (kind 0x0D) spawns nothing.
#[test]
fn the_finisher_air_down_lands_with_a_damaging_shockwave() {
    let bench = air_down(0x0E);
    assert_eq!(bench.sora.combat().shots_spawned, 1);
    assert!(bench.effects().iter().any(|e| e.1 == "vp_hit_zmn0"));
    let bomb: Vec<_> = bench.host.hits().into_iter().filter(|(r, _)| r.attack_name == "KB_DOWN_BOMB").collect();
    assert_eq!(bomb.len(), 1, "the shockwave hits the dummy once: {:?}", bench.host.hits().iter().map(|h| h.0.attack_name).collect::<Vec<_>>());
    assert_eq!((bomb[0].0.damage_kind, bomb[0].0.target), (Kind::Blow, TargetId(1)));
    assert!(bench.sora.combat().shots.is_empty(), "the shot is gone after its 0.5 s");
    let bench = air_down(0x0D);
    assert_eq!(bench.sora.combat().shots_spawned, 0, "kind 0x0D has no case in the shot handler");
}

/// Spec 5.6.3 / 8.4 (request interface slots 138 - 140): a hit that his body answers with
/// GUARD_REFLECT becomes the guard reaction - a successful guard-hit entry with the counter
/// window - when the counter-allowed gate passes (here: out of an attack), else the recoil.
#[test]
fn a_guard_reflect_result_on_him_is_the_guard_reaction_out_of_an_attack() {
    // (The host resolved the hit against his body with result 4: `HitOnPlayer::reaction_type`.)
    let hit = HitOnPlayer { reaction_type: 4, ..status_hit(Kind::Small, 10.0, "NONE", 0.0) };
    let mut bench = Bench::new(level99(None), &[dummy(1, 200.0, 0.0, 0.0)]);
    let hp = bench.sora.status().hp;
    bench.tap(PadButton::ATTACK);
    bench.until(10, "the attack state", |_, o| o.state == StateId::Attack);
    bench.sora.take_hit(hit);
    bench.until(5, "the guard reaction", |_, o| o.state == StateId::Guard);
    assert!(bench.sora.actions().counter_window > 0.0, "the 1.0 s counter window is open");
    assert_eq!(bench.sora.status().hp, hp, "no HP loss");
    // Out of Idle the gate refuses (state 1 is not in its list): the weapon recoil.
    let mut bench = Bench::new(level99(None), &[dummy(1, 200.0, 0.0, 0.0)]);
    bench.sora.take_hit(hit);
    bench.until(5, "the recoil", |_, o| o.state == StateId::Reflect);
}

// ------------------------------------------------------------------------------------ item 4: abilities

/// Guards a hit of dummy 1 and answers with the guard counter; returns the bench when the
/// counter's first hit has landed.
fn counter_landed(equipped: Option<&[&str]>) -> Bench {
    let mut bench = Bench::new(level99(equipped), &[dummy(1, 200.0, 0.0, 0.0)]);
    bench.tap(PadButton::GUARD);
    bench.idle(10);
    bench.sora.take_hit(HitOnPlayer { direction: Vec3::new(-1.0, 0.0, 0.0), causer_yaw: 180.0, ..HitOnPlayer::new(Kind::Small, 30.0, TargetId(1), Vec3::new(200.0, 0.0, 0.0)) });
    bench.step(&[], &[]);
    assert!(bench.sora.actions().counter_window > 0.0, "the counter window is open");
    bench.tap(PadButton::ATTACK);
    bench.until(120, "the counter's hit", |b, _| !b.host.hits().is_empty());
    bench
}

/// REVENGE_EX, Final Blow (spec 5.6.4): a guard counter that hits its target with the ability
/// arms the chase while the target is in a blow reaction (the reaction command 12 is shown); the
/// next attack press is the chase attack, kind 0x22: after 0.2 s he is teleported to 150 cm from
/// the target and `ChaseAttack` plays. Without the ability the press is an ordinary combo step.
#[test]
fn final_blow_chases_the_target_of_a_guard_counter() {
    let mut bench = counter_landed(None);
    let counter = bench.sora.attack().expect("the counter runs").kind;
    assert!(matches!(counter, 0x21 | 0x23 | 0x24));
    assert!(bench.sora.combat().chase_armed && bench.sora.combat().revenge == TargetId(1), "armed: the dummy is in reaction {:?}", bench.host.reactions);
    assert!(bench.events().any(|e| matches!(e, EventKind::SituationSlot { kind, .. } if kind.value() == 12)), "the Final Blow prompt");
    // Put the dummy somewhere else, as a blown-away enemy would be: the chase warps to it.
    bench.host.targets[0].position = Vec3::new(900.0, 300.0, 0.0);
    bench.host.targets[0].center = Vec3::new(900.0, 300.0, 90.0);
    for _ in 0..90 {
        let out = bench.tap(PadButton::ATTACK);
        bench.step(&[], &[]);
        if bench.sora.attack().is_some_and(|a| a.kind == 0x22) || out.state == StateId::Idle {
            break;
        }
    }
    assert_eq!(bench.sora.attack().map(|a| a.kind), Some(0x22), "the chase attack; states {:?}", bench.states());
    assert_eq!(bench.sora.combat().chases, 1);
    bench.until(30, "the chase clip", |_, o| o.anim.name.name() == "ChaseAttack");
    let at = bench.outs.last().unwrap().position;
    let gap = (Vec3::new(900.0, 300.0, 0.0) - at).truncate().length();
    assert!((gap - 150.0).abs() < 25.0, "he stands {gap} cm from the target after the warp ({at:?})");
    bench.until(300, "Idle", |_, o| o.state == StateId::Idle);
    assert!(bench.host.hits().iter().any(|(r, _)| r.attack_name.starts_with("REVENGE_EX")), "the chase attack's rows hit: {:?}", bench.host.hits().iter().map(|h| h.0.attack_name).collect::<Vec<_>>());
    // Without REVENGE_EX: no chase.
    let without: Vec<&str> = vec!["REFLECT_GUARD", "GUARD_COUNTER", "COMBO_MASTER"];
    let bench = counter_landed(Some(&without));
    assert!(!bench.sora.combat().chase_armed && bench.sora.combat().revenge == TargetId::NONE);
}

/// AUTO_FINISH (ability 70; spec 11.5, HUD 0x141618830): in battle, half a second before the
/// form's time is out, the finish command in slot B is executed by itself. Without the ability
/// the form just ends.
#[test]
fn auto_finish_fires_the_finish_command_before_the_form_runs_out() {
    for (equipped, expect) in [(vec!["AUTO_FINISH"], true), (vec![], false)] {
        let mut bench = Bench::new(level99(Some(&equipped)), &[dummy(1, 300.0, 0.0, 0.0)]);
        bench.sora.force_battle(true);
        bench.sora.debug_add_style(100.0);
        bench.idle(15);
        bench.tap(PadButton::SITUATION);
        bench.until(400, "Second Form and control back", |b, o| b.sora.combat().form.cmd != 0 && o.state == StateId::Idle);
        assert!(bench.sora.combat().situation.slot_b.is_some(), "the finish command is offered");
        bench.sora.combat_mut().form.time_left = 1.0;
        bench.idle(50);
        let finished = bench.states().contains(&StateId::Stylefinish);
        assert_eq!((finished, bench.sora.combat().situation.auto_finished), (expect, expect as u32), "equipped {equipped:?}: states {:?}", bench.states());
    }
}

/// CHARGE_BERSERK (ability 178; Attack Begin 0x1413333BC): while the MP charge runs, outside a
/// form, the combo does not reach its finisher: every step is a normal step. Without the charge
/// the fourth attack is the finisher (kind 4).
#[test]
fn berserk_charge_keeps_the_combo_going_while_mp_recharges() {
    let run = |charging: bool| {
        let mut status = level99(Some(&["COMBO_MASTER"]));
        status.abilities.add_worn(kh_core::enums::ETresAbilityKind::ChargeBerserk);
        let mut bench = Bench::new(status, &[dummy(1, 150.0, 0.0, 0.0)]);
        if charging {
            bench.sora.status_mut().set_mp_charge(true, 0.0);
        }
        for _ in 0..600 {
            let out = bench.tap(PadButton::ATTACK);
            bench.step(&[], &[]);
            if bench.sora.combat().history.len() >= 7 || (bench.outs.len() > 40 && out.state == StateId::Idle) {
                break;
            }
        }
        bench.sora.combat().history.clone()
    };
    let plain = run(false);
    assert_eq!(plain.get(3), Some(&4), "the fourth attack is the finisher: {plain:x?}");
    let endless = run(true);
    assert!(endless.len() >= 7 && !endless[..7].contains(&4), "no finisher while charging: {endless:x?}");
}

/// AIR_ROLL_BEAT (Groundbreaker) and AIRCOMBO_PLUS, through the combo decision (spec 4.1, 4.2
/// rule 5): the jump press during an air combo with the Groundbreaker stock is launch kind 0x1D
/// (once per chain); each Air Combo Plus adds one step before the finisher in the air only.
#[test]
fn groundbreaker_and_air_combo_plus_in_the_decision() {
    use kh_core::combat::{decide, ComboAbilities, DecideIn};
    let abilities = ComboAbilities { air_roll_beat: 1, air_combo_plus: 2, ..ComboAbilities::NONE };
    assert_eq!((abilities.max_combo(false), abilities.max_combo(true)), (6, 4), "Air Combo Plus x2 lengthens the air combo only");
    let step = |combo: i32, launch: bool, stocks: (i32, i32)| DecideIn { combo, reach: 0, last_kind: 0x10, dist: 100.0, dz: 0.0, target_high: false, target_on_ground: false, air: true, num_enemies: 1, launch, endless: false, max_combo: abilities.max_combo(false), finishers: abilities.finishers(), finish_count: 0, stocks };
    let launch = decide(&step(1, true, (0, 1)), &abilities);
    assert_eq!((launch.kind, launch.stocks.1, launch.combo), (0x1D, 2, 0), "Groundbreaker: the stock is used, the combo restarts");
    // Steps 0..4 of the six-step air combo are normal steps; the sixth (combo 5) is the finisher.
    for combo in 1..5 {
        assert!(!decide(&step(combo, false, (0, 2)), &abilities).finish, "step {combo}");
    }
    assert!(decide(&step(5, false, (0, 2)), &abilities).finish);
}

/// SONIC_DOWN, Sonic Cleave (flowmotion attack kind 0x37; shot handler case 0x37, spec 8.8): its
/// shot `SHOCKWAVE_S` (row `FF_SONIC_BOMB`, a sphere growing from 100 to 350 cm for 0.5 s) is
/// spawned once: at the cleave's first hit, or when it has landed and its Shot notify has fired.
#[test]
fn sonic_cleave_spawns_its_shockwave_once() {
    let bench = air_down(0x37);
    assert_eq!(bench.sora.combat().shots_spawned, 1);
    assert!(bench.effects().iter().any(|e| e.1 == "vp_snc_hit0"));
    let names: Vec<_> = bench.host.hits().iter().map(|h| h.0.attack_name).collect();
    assert_eq!(names.iter().filter(|n| **n == "FF_SONIC_BOMB").count(), 1, "{names:?}");
}

// ------------------------------------------------------------------------------------ attacks out of the traversal states

/// Jumps, air-slides along +X onto a pole / bar at x = 900 and returns in the PoleTurn state.
fn on_the_pole(horizontal: bool) -> Bench {
    use kh_core::world::Pole;
    let mut world = TestWorld::floor();
    world.poles.push(if horizontal {
        Pole { id: 7, base1: Vec3::new(900.0, -200.0, 500.0), base2: Vec3::new(900.0, 200.0, 500.0), check_radius: 50.0, attach_radius: 30.0, horizontal: true, active: true }
    } else {
        Pole { id: 7, base1: Vec3::new(900.0, 0.0, 150.0), base2: Vec3::new(900.0, 0.0, 1100.0), check_radius: 100.0, attach_radius: 90.0, horizontal: false, active: true }
    });
    let mut bench = Bench::in_world(level99(None), &[dummy(1, 1300.0, 0.0, 0.0)], world);
    bench.step(&[PadButton::JUMP], &[]);
    for _ in 0..120 {
        let out = bench.step(&[PadButton::JUMP], &[PadButton::JUMP]);
        if out.state == StateId::Jump && out.sub_step == 4 {
            break;
        }
    }
    bench.step_with(&[PadButton::GUARD], &[], Vec2::new(0.0, 1.0));
    bench.until(60, "the pole", |_, o| o.state == StateId::PoleTurn);
    bench.idle(20);
    assert_eq!(bench.outs.last().unwrap().state, StateId::PoleTurn);
    bench
}

/// TryAttack out of POLE_TURN (0x141437330 case 0x17; spec 3.3): an attack press on a bar swing
/// is the Somersault (kind 0x39, clip 1184, ability SUMMERSALT); on a pole spin it is a flow
/// attack forced as 0x38 (Buzz Saw; refined by the abilities and the situation).
#[test]
fn attacks_out_of_a_bar_swing_and_a_pole_spin() {
    let mut bench = on_the_pole(true);
    bench.tap(PadButton::ATTACK);
    bench.until(6, "the attack state", |_, o| o.state == StateId::Attack);
    assert_eq!(bench.sora.attack().map(|a| (a.kind, a.target)), Some((0x39, TargetId::NONE)), "the Somersault has no target");
    assert_eq!(bench.outs.last().unwrap().anim.name.name(), "1184");
    let mut bench = on_the_pole(false);
    bench.tap(PadButton::ATTACK);
    bench.until(6, "the attack state", |_, o| o.state == StateId::Attack);
    let kind = bench.sora.attack().unwrap().kind;
    assert!(matches!(kind, 0x36 | 0x37 | 0x38), "a flow attack out of the pole spin: {kind:#x}");
}

/// `CanAttack` rows of the action states (spec 3.2): out of a Dodge Roll an attack starts from
/// `TresEnableChange` on, not before.
#[test]
fn an_attack_leaves_a_dodge_roll_at_its_change_point() {
    let mut bench = Bench::new(level99(None), &[dummy(1, 600.0, 0.0, 0.0)]);
    bench.step_with(&[PadButton::GUARD], &[], Vec2::new(0.0, 1.0));
    bench.until(4, "the roll", |_, o| o.state == StateId::Dodgeroll);
    bench.tap(PadButton::ATTACK);
    bench.idle(3);
    assert_eq!(bench.outs.last().unwrap().state, StateId::Dodgeroll, "too early: refused");
    let mut started = None;
    for n in 0..80 {
        bench.tap(PadButton::ATTACK);
        let out = bench.step(&[], &[]);
        if out.state == StateId::Attack {
            started = Some(n);
            break;
        }
        assert_eq!(out.state, StateId::Dodgeroll, "nothing but the roll or the attack");
    }
    assert!(started.is_some(), "the attack out of the roll");
}
