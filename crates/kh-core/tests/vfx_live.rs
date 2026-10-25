//! (vfx implementer 4) Every PERSISTENT effect the core starts is ended by the core in every
//! exit path: after each sequence below the set of persistent handles that were spawned and not
//! stopped is empty. (The user saw a glow that stayed on the Keyblade after a shotlock; that one
//! was the effect evaluator's, but this is the core's half of the same promise.)

use std::collections::BTreeMap;

use kh_core::command::CommandRequest;
use kh_core::enums::ETresDamageKind as Kind;
use kh_core::host::EventKind;
use kh_core::magic::spells::cmd;
use kh_core::magic::View;
use kh_core::states::HitOnPlayer;
use kh_core::status::LoadoutValue;
use kh_core::test_world::TestWorld;
use kh_core::{Input, Loadout, Output, PadButton, PadButtons, PadState, Pose, RecordingHost, Sora, StateId, Status, TargetFlags, TargetId, TargetInfo, Tuning, Vec2, Vec3, SORA_PARAMS};

const DT: f32 = 1.0 / 60.0;

fn level99() -> Status {
    let path = format!("{}/../../sheets/port/sora-loadout-99.json", env!("CARGO_MANIFEST_DIR"));
    let json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let mut loadout = Loadout::new("sora-loadout-99");
    for row in json["rows"].as_array().unwrap() {
        let key = row[0].as_str().unwrap();
        let value = match &row[1] {
            serde_json::Value::Number(n) => LoadoutValue::Number(n.as_f64().unwrap()),
            serde_json::Value::String(s) => LoadoutValue::Text(s.clone()),
            serde_json::Value::Array(items) => LoadoutValue::List(items.iter().map(|i| i.as_str().unwrap().to_string()).collect()),
            serde_json::Value::Object(map) => LoadoutValue::Map(map.iter().map(|(k, v)| (k.clone(), v.as_f64().unwrap())).collect()),
            other => panic!("{key}: {other}"),
        };
        loadout.set(key, &value).unwrap();
    }
    loadout.resolve().unwrap();
    Status::from_loadout(&loadout)
}

fn dummy(id: u32, x: f32, y: f32) -> TargetInfo {
    TargetInfo { id: TargetId(id), position: Vec3::new(x, y, 0.0), center: Vec3::new(x, y, 90.0), velocity: Vec3::ZERO, yaw: 180.0, radius: 40.0, half_height: 90.0, flags: TargetFlags(TargetFlags::ALIVE | TargetFlags::HITTABLE | TargetFlags::LOCKABLE | TargetFlags::ENEMY), hp: 1.0e7, hp_max: 1.0e7 }
}

struct Bench {
    sora: Sora,
    host: RecordingHost,
    world: TestWorld,
    held: PadButtons,
    stick: Vec2,
    /// Persistent handles spawned and not stopped yet, with the effect's name.
    open: BTreeMap<u32, &'static str>,
    /// Every persistent effect ever started (to prove a sequence exercised something).
    started: Vec<&'static str>,
}

impl Bench {
    fn new() -> Bench {
        let origin = Vec3::new(0.0, 0.0, Sora::origin_height_on_floor(&SORA_PARAMS));
        let sora = Sora::new(SORA_PARAMS, Pose { position: origin, yaw: 0.0 }).with_tuning(Tuning::GAME_EXACT).with_status(level99());
        let mut host = RecordingHost::new();
        host.targets.extend_from_slice(&[dummy(1, 450.0, 0.0), dummy(2, 500.0, 200.0), dummy(3, 500.0, -200.0)]);
        let mut bench = Bench { sora, host, world: TestWorld::floor(), held: PadButtons::NONE, stick: Vec2::ZERO, open: BTreeMap::new(), started: Vec::new() };
        bench.run(5);
        bench
    }

    fn step(&mut self, held: &[PadButton]) -> Output {
        let now = held.iter().fold(PadButtons::NONE, |all, b| all.with(*b));
        let pad = PadState::from_held(now, self.held);
        self.held = now;
        let mut input = Input::default().with_pad(pad);
        input.view = Some(View::from_camera(Vec3::new(-300.0, 0.0, 150.0), 0.0, 0.0, 90.0, 1280, 720));
        input.move_stick = self.stick;
        let (sora, world) = (&mut self.sora, &self.world);
        let out = self.host.step(world, |h| sora.step_hosted(DT, &input, h));
        for event in &out.events {
            match &event.kind {
                EventKind::EffectSpawn { handle, spawn } if spawn.persistent => {
                    self.open.insert(handle.0, spawn.name);
                    self.started.push(spawn.name);
                }
                EventKind::EffectStop { handle, .. } => {
                    self.open.remove(&handle.0);
                }
                _ => {}
            }
        }
        out
    }

    fn run(&mut self, steps: usize) -> Output {
        self.hold(&[], steps)
    }

    fn hold(&mut self, held: &[PadButton], steps: usize) -> Output {
        let mut out = self.sora.output();
        for _ in 0..steps {
            out = self.step(held);
        }
        out
    }

    fn cast(&mut self, kind: u8) {
        self.sora.command_request(CommandRequest::Execute(kind));
        self.step(&[]);
    }

    fn hit(&mut self, kind: Kind, status: &'static str, seconds: f32) {
        let own = self.sora.output().position;
        self.sora.take_hit(HitOnPlayer { direction: Vec3::new(-1.0, 0.0, 0.0), causer_yaw: 180.0, bad_status: status, bad_time: seconds, ..HitOnPlayer::new(kind, 10.0, TargetId(1), own + Vec3::new(150.0, 0.0, 0.0)) });
    }

    /// Lets everything run out, then: nothing persistent is left.
    fn settle(&mut self, what: &str) {
        self.run(900);
        assert!(self.open.is_empty(), "{what}: persistent effects never stopped: {:?} (state {:?})", self.open, self.sora.output().state);
        self.sora.status_mut().full_recover();
    }
}

const SPELLS: [u8; 18] = [cmd::FIRE, cmd::FIRA, cmd::FIRAGA, cmd::BLIZZARD, cmd::BLIZZARA, cmd::BLIZZAGA, cmd::THUNDER, cmd::THUNDARA, cmd::THUNDAGA, cmd::WATER, cmd::WATARA, cmd::WATAGA, cmd::AERO, cmd::AERORA, cmd::AEROGA, cmd::CURE, cmd::CURA, cmd::CURAGA];

#[test]
fn every_spell_ends_its_persistent_effects() {
    let mut b = Bench::new();
    for kind in SPELLS {
        let before = b.started.len();
        b.cast(kind);
        b.run(20);
        assert!(b.started.len() > before, "command {kind}: the cast started no persistent effect (cast effect / spell actor)");
        b.settle(&format!("command {kind}"));
    }
}

#[test]
fn a_cast_cut_by_damage_ends_its_effects() {
    for (kind, status, seconds) in [(Kind::Small, "NONE", 0.0), (Kind::Blow, "NONE", 0.0), (Kind::Small, "FREEZE", 2.0), (Kind::Small, "STUN", 2.0), (Kind::Small, "BURN", 2.0), (Kind::Small, "SNEEZE", 2.0)] {
        for spell in [cmd::FIRAGA, cmd::THUNDAGA, cmd::WATAGA, cmd::CURAGA] {
            // The cast effect is on the Keyblade from the first step of the cast: cut it early.
            for delay in [2usize, 8, 20] {
                let mut b = Bench::new();
                b.cast(spell);
                b.run(delay);
                b.hit(kind, status, seconds);
                b.settle(&format!("spell {spell} cut after {delay} steps by {kind:?} / {status}"));
            }
        }
    }
}

#[test]
fn bad_statuses_end_their_effects() {
    for status in ["FREEZE", "STUN", "BURN", "SNEEZE", "STOP", "HONEY", "CLOUD", "DISCHARGE"] {
        let mut b = Bench::new();
        b.hit(Kind::Small, status, 2.0);
        b.settle(status);
        // And cut short by a second hit.
        b.hit(Kind::Small, status, 3.0);
        b.run(30);
        b.hit(Kind::Blow, "NONE", 0.0);
        b.settle(&format!("{status} then a blow"));
    }
}

#[test]
fn shotlock_paths_end_their_effects() {
    // King of Hearts (a short aim), Ragnarok (the aim held until the locks are full), the aim
    // let go, the airstep out of the aim, and a volley cut by nothing else (he is invincible).
    for (aim_steps, what) in [(150usize, "King of Hearts"), (480, "Ragnarok")] {
        let mut b = Bench::new();
        b.hold(&[PadButton::RightShoulder], aim_steps);
        b.step(&[PadButton::RightShoulder, PadButton::ATTACK]);
        let mut fired = false;
        for _ in 0..40 {
            fired |= b.step(&[PadButton::RightShoulder]).state == StateId::Shootflow;
        }
        assert!(fired, "{what}: the shotlock did not start");
        b.settle(what);
        assert!(b.started.iter().any(|n| n.starts_with("TresProjKC")), "{what}: no volley: {:?}", b.started);
    }
    let mut b = Bench::new();
    b.hold(&[PadButton::RightShoulder], 90);
    b.settle("aim let go");
    b.hold(&[PadButton::RightShoulder], 40);
    assert!(b.sora.aim().can_airstep, "no airstep target");
    b.step(&[PadButton::RightShoulder, PadButton::GUARD]);
    let mut flew = false;
    for _ in 0..10 {
        flew |= b.step(&[]).state == StateId::Athleticflow;
    }
    assert!(flew, "the airstep did not start");
    b.settle("airstep out of the aim");
    // The airstep's flow attack, and an airstep cut by a hit on arrival.
    b.sora.teleport(Pose { position: Vec3::new(0.0, 0.0, Sora::origin_height_on_floor(&SORA_PARAMS)), yaw: 0.0 });
    b.run(5);
    b.hold(&[PadButton::RightShoulder], 40);
    b.step(&[PadButton::RightShoulder, PadButton::GUARD]);
    for _ in 0..200 {
        if b.step(&[PadButton::ATTACK]).state == StateId::Attack {
            break;
        }
        b.step(&[]);
    }
    b.settle("airstep then the flow attack");
}

#[test]
fn form_guard_dodge_glide_and_combo_end_their_effects() {
    let mut b = Bench::new();
    // Second Form: the charge effect on the Keyblade, until the form runs out by itself.
    b.sora.debug_add_style(100.0);
    b.run(15);
    b.step(&[PadButton::SITUATION]);
    b.run(60);
    assert!(b.started.contains(&"vs_com_lv2"), "the form's charge effect: {:?}", b.started);
    // A hit while the form is up, a cast in the form, then the form's own end.
    b.hit(Kind::Blow, "NONE", 0.0);
    b.run(120);
    b.cast(cmd::FIRAGA);
    for _ in 0..8000 {
        b.step(&[]);
        if b.open.is_empty() {
            break;
        }
    }
    b.settle("Second Form running out");
    // The trail of the combo (a state notify), cut by a guard, a dodge and a jump + glide.
    for cut in [PadButton::GUARD, PadButton::JUMP] {
        for delay in [6usize, 14, 22] {
            b.step(&[PadButton::ATTACK]);
            b.run(delay);
            b.step(&[cut]);
            b.hold(&[cut], 40);
            b.settle(&format!("combo cut by {cut:?} after {delay} steps"));
        }
    }
    b.stick = Vec2::new(0.0, 1.0);
    b.step(&[PadButton::GUARD]);
    b.run(10);
    b.stick = Vec2::ZERO;
    b.hit(Kind::Small, "NONE", 0.0);
    b.settle("dodge roll cut by a hit");
    b.step(&[PadButton::JUMP]);
    b.run(12);
    b.step(&[PadButton::JUMP]);
    b.hold(&[PadButton::JUMP], 30);
    b.hit(Kind::Blow, "NONE", 0.0);
    b.settle("glide cut by a blow");
}

/// The Water bolt is LOBBED (the user's capture of the real game; magic spec B 3.4 / I 3.1): the
/// barrier rides on Sora pitched 60 degrees up, the bolt leaves along that axis at 1400 cm/s with
/// no gravity and is turned by its homing (toward the target, or toward the ground without one):
/// its height rises, then falls, and it ends at the target / on the floor ahead of him.
#[test]
fn the_water_bolt_is_lobbed() {
    for (with_target, tier) in [(true, cmd::WATER), (true, cmd::WATAGA), (false, cmd::WATER), (false, cmd::WATAGA)] {
        let mut b = Bench::new();
        if with_target {
            b.host.targets.retain(|t| t.id == TargetId(1));
            b.host.targets[0].position = Vec3::new(900.0, 0.0, 0.0);
            b.host.targets[0].center = Vec3::new(900.0, 0.0, 90.0);
        } else {
            b.host.targets.clear();
        }
        b.run(3);
        b.cast(tier);
        let mut path: Vec<Vec3> = Vec::new();
        for _ in 0..600 {
            b.step(&[]);
            if let Some(p) = b.sora.magic().projectile_views().find(|p| p.class.ends_with("Blt_C")) {
                path.push(p.position);
            } else if !path.is_empty() {
                break;
            }
        }
        assert!(path.len() > 5, "target {with_target}, command {tier}: no bolt flew ({} samples)", path.len());
        let start = path[0];
        let top = path.iter().map(|p| p.z).fold(f32::MIN, f32::max);
        let end = *path.last().unwrap();
        let apex = path.iter().position(|p| p.z == top).unwrap();
        assert!(top > start.z + 40.0, "target {with_target}, command {tier}: it rises first (start z {:.0}, top {top:.0})", start.z);
        assert!(apex > 0 && apex < path.len() - 1, "target {with_target}, command {tier}: the top is inside the flight (sample {apex} of {})", path.len());
        assert!(end.z < top - 40.0, "target {with_target}, command {tier}: then it falls (top {top:.0}, end z {:.0})", end.z);
        if with_target {
            assert!((end.truncate() - Vec3::new(900.0, 0.0, 0.0).truncate()).length() < 150.0 && end.z < 260.0, "it lands on the target 9 m ahead: ended at {end:?}");
        } else {
            assert!(end.x > start.x + 100.0 && end.z < 120.0, "without a target it comes down on the floor ahead: ended at {end:?}");
        }
    }
}
