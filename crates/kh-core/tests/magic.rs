//! Magic, items, the command model, shotlock and airstep against training dummies:
//! docs/sora-magic-spec.md part K (the test list with the level-99 numbers: MAG 67 = 64 + the
//! Kingdom Key's 3). Each test cites its T-number. Everything is driven through the pad
//! (`Input::pad`) or through `Sora::command_request`, which runs the same `ExecuteCommand`.

use kh_core::command::{CommandRequest, MenuEvent, MenuState};
use kh_core::enums::{ETresAbilityKind, ETresDamageKind};
use kh_core::host::{EventKind, HitRecord};
use kh_core::magic::spells::cmd;
use kh_core::magic::{View, MASK_PLAYER};
use kh_core::status::{ItemKind, LoadoutValue};
use kh_core::test_world::TestWorld;
use kh_core::{AnimId, Input, Loadout, Output, PadButton, PadButtons, PadState, Pose, RecordingHost, Sora, StateId, Status, TargetFlags, TargetId, TargetInfo, Tuning, Vec3, SORA_PARAMS};

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

/// The level-99 loadout with exactly these abilities equipped (T0: none unless a test says so).
fn level99(equipped: &[&str]) -> Status {
    let path = format!("{}/../../sheets/port/sora-loadout-99.json", env!("CARGO_MANIFEST_DIR"));
    let json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))).unwrap();
    let mut loadout = Loadout::new("sora-loadout-99");
    for row in json["rows"].as_array().unwrap() {
        let key = row[0].as_str().unwrap();
        let value = if key == "abilities.equipped" { LoadoutValue::List(equipped.iter().map(|s| s.to_string()).collect()) } else { cell(&row[1]).unwrap() };
        loadout.set(key, &value).unwrap_or_else(|e| panic!("{e}"));
    }
    loadout.resolve().unwrap();
    Status::from_loadout(&loadout)
}

fn dummy(id: u32, x: f32, y: f32) -> TargetInfo {
    TargetInfo { id: TargetId(id), position: Vec3::new(x, y, 0.0), center: Vec3::new(x, y, 90.0), velocity: Vec3::ZERO, yaw: 180.0, radius: 40.0, half_height: 90.0, flags: TargetFlags(TargetFlags::ALIVE | TargetFlags::HITTABLE | TargetFlags::LOCKABLE | TargetFlags::ENEMY), hp: 1.0e6, hp_max: 1.0e6 }
}

struct Bench {
    sora: Sora,
    host: RecordingHost,
    world: TestWorld,
    held: PadButtons,
    /// A view from behind Sora looking along +X (the reticle test's camera).
    view: Option<View>,
    time: f32,
}

impl Bench {
    fn new(status: Status, dummies: &[TargetInfo]) -> Bench {
        let origin = Vec3::new(0.0, 0.0, Sora::origin_height_on_floor(&SORA_PARAMS));
        let sora = Sora::new(SORA_PARAMS, Pose { position: origin, yaw: 0.0 }).with_tuning(Tuning::GAME_EXACT).with_status(status);
        let mut host = RecordingHost::new();
        host.targets.extend_from_slice(dummies);
        let mut bench = Bench { sora, host, world: TestWorld::floor(), held: PadButtons::NONE, view: Some(View::from_camera(Vec3::new(-300.0, 0.0, 150.0), 0.0, 0.0, 90.0, 1280, 720)), time: 0.0 };
        bench.run(5);
        bench
    }

    /// One step with these buttons down.
    fn step(&mut self, held: &[PadButton]) -> Output {
        let now = held.iter().fold(PadButtons::NONE, |all, b| all.with(*b));
        let pad = PadState::from_held(now, self.held);
        self.held = now;
        let mut input = Input::default().with_pad(pad);
        input.view = self.view;
        let (sora, world) = (&mut self.sora, &self.world);
        self.time += DT;
        self.host.step(world, |h| sora.step_hosted(DT, &input, h))
    }

    fn run(&mut self, steps: usize) -> Vec<Output> {
        (0..steps).map(|_| self.step(&[])).collect()
    }

    fn hold(&mut self, held: &[PadButton], steps: usize) -> Vec<Output> {
        (0..steps).map(|_| self.step(held)).collect()
    }

    /// Press and release.
    fn tap(&mut self, button: PadButton) {
        self.step(&[button]);
        self.step(&[]);
    }

    fn cast(&mut self, kind: u8) -> Output {
        self.sora.command_request(CommandRequest::Execute(kind));
        self.step(&[])
    }

    /// Steps until the state is `id` (at most `max` steps); the steps taken.
    fn until_state(&mut self, id: StateId, max: usize) -> usize {
        for n in 0..max {
            if self.step(&[]).state == id {
                return n + 1;
            }
        }
        panic!("the state never became {id:?}");
    }

    fn hits(&self) -> Vec<HitRecord> {
        self.host.hits().into_iter().map(|h| h.0).collect()
    }

    fn status(&mut self) -> &mut Status {
        self.sora.status_mut()
    }
}

fn seconds(steps: usize) -> f32 {
    steps as f32 * DT
}

// ------------------------------------------------------------------------------------ magic

/// T4 + T5 (Fire): the timeline of a standing Fire at a dummy 800 cm ahead, and its damage.
#[test]
fn fire_cast_timeline_and_damage() {
    let mut b = Bench::new(level99(&[]), &[dummy(1, 800.0, 0.0)]);
    b.status().set_mp(100);
    b.cast(cmd::FIRE);
    // The request was made in the UI phase of that step: the state begins in the next.
    let out = b.step(&[]);
    assert_eq!((out.state, out.anim.name), (StateId::Firemagic, AnimId::MagicFire));
    let cast = b.sora.cast().unwrap();
    assert_eq!((cast.cmd, cast.combo_index, cast.bullets), (cmd::FIRE, 0, 0));
    assert_eq!(out.velocity, Vec3::ZERO);
    // The shot: 0.3 s into the clip. MP -10 at the shot, not before.
    let mut shot_at = 0;
    for n in 1..60 {
        b.step(&[]);
        if b.sora.cast().is_some_and(|c| c.bullets == 1) {
            shot_at = n;
            break;
        }
        assert_eq!(b.sora.status().mp, 100, "MP is deducted at the shot");
    }
    assert!((seconds(shot_at) - 0.3).abs() <= 2.0 * DT, "shot after {} s", seconds(shot_at));
    assert_eq!(b.sora.status().mp, 90);
    let bolt = b.sora.magic().projectile_views().next().expect("the bolt");
    assert_eq!(bolt.class, "TresProj_MagFireNml_C");
    // From the Keyblade's tip: in front of him, above the floor.
    assert!(bolt.position.x > 20.0 && bolt.position.z > 60.0 && bolt.position.z <= 163.0 + 5.0, "{:?}", bolt.position);
    // 2000 cm/s: about 0.4 s to the dummy; one hit of 67, row MAG_FIRE; no explosion actor.
    let mut hit_at = 0;
    for n in 1..60 {
        b.step(&[]);
        if !b.hits().is_empty() {
            hit_at = n;
            break;
        }
    }
    assert!((seconds(hit_at) - 0.37).abs() < 0.08, "hit after {} s", seconds(hit_at));
    let hit = b.hits()[0];
    assert_eq!((hit.attack_name, hit.damage, hit.damage_kind), ("MAG_FIRE", 67.0, ETresDamageKind::Small));
    assert_eq!(hit.flags & 0xFF, cmd::FIRE as u32);
    b.run(3);
    assert_eq!(b.sora.magic().projectile_views().count(), 0, "tier 1 has no burn actor");
    // The clip ends at 1.2 s: Idle again.
    let end = b.until_state(StateId::Idle, 90);
    let total = seconds(1 + shot_at + hit_at + 3 + end);
    assert!((total - 1.2).abs() < 0.1, "the state lasted {total} s");
    assert_eq!(b.hits().len(), 1);
}

/// T5: the damage of every tier on the T0 dummy, and Fira / Firaga's explosion.
#[test]
fn spell_damage_table() {
    for (kind, row, damage) in [
        (cmd::FIRA, "MAG_FIRA", 101.0),
        (cmd::FIRAGA, "MAG_FIGA", 148.0),
        (cmd::BLIZZARD, "MAG_BLIZZARD", 54.0),
        (cmd::BLIZZARA, "MAG_BLIZZARA", 81.0),
        (cmd::BLIZZAGA, "MAG_BLIZZAGA", 121.0),
        (cmd::THUNDER, "MAG_THUNDER", 81.0),
        (cmd::THUNDARA, "MAG_THUNDARA", 108.0),
        (cmd::THUNDAGA, "MAG_THUNDAGA", 161.0),
        (cmd::AERO, "MAG_AERO", 41.0),
        (cmd::AERORA, "MAG_AERORA", 67.0),
        (cmd::AEROGA, "MAG_AEROGA", 94.0),
    ] {
        let mut b = Bench::new(level99(&[]), &[dummy(1, 600.0, 0.0)]);
        b.status().set_mp(100);
        b.cast(kind);
        b.run(150);
        let hits = b.hits();
        assert_eq!(hits.len(), 1, "{row}: one hit per enemy");
        assert_eq!((hits[0].attack_name, hits[0].damage), (row, damage), "{row}");
    }
}

/// T5: Firaga's burn hits the enemies around the one the bolt hit, once, not that one again.
#[test]
fn firaga_explosion() {
    let mut b = Bench::new(level99(&[]), &[dummy(1, 600.0, 0.0), dummy(2, 700.0, 150.0), dummy(3, 600.0, 900.0)]);
    b.status().set_mp(100);
    b.cast(cmd::FIRAGA);
    b.run(150);
    let hits = b.hits();
    let on = |id: u32| hits.iter().filter(|h| h.target == TargetId(id)).count();
    assert_eq!((on(1), on(2), on(3)), (1, 1, 0), "{:?}", hits.iter().map(|h| (h.target, h.attack_name)).collect::<Vec<_>>());
    assert!(hits.iter().all(|h| h.damage == 148.0));
}

/// T5: Blizzard applies FREEZE (100 %, 5 s) and lays rail nodes on the floor under its path.
#[test]
fn blizzard_freeze_and_rail() {
    let mut b = Bench::new(level99(&[]), &[dummy(1, 1500.0, 0.0)]);
    b.status().set_mp(100);
    b.cast(cmd::BLIZZARD);
    let mut most = 0;
    for _ in 0..90 {
        b.step(&[]);
        most = most.max(b.sora.magic().rails.len());
    }
    let hit = b.hits()[0];
    assert_eq!((hit.info.bad_status, hit.info.bad_time, hit.damage), ("FREEZE", 5.0, 54.0));
    // Every 300 cm of travel (the first after 35): about 5 nodes over 1400 cm, each on the floor.
    assert!((4..=6).contains(&most), "{most} rail nodes");
    b.run(200);
    assert!(b.sora.magic().rails.is_empty(), "a node lives 3 s");
}

/// T5 / part I 1, 2: Thunder and Aero hit every enemy inside once; Aero swirls the first 3 and
/// blows the rest away.
#[test]
fn thunder_and_aero_against_several() {
    let around: Vec<TargetInfo> = (0..5).map(|k| dummy(k + 1, 500.0 + 60.0 * (k as f32 - 2.0), 80.0 * (k as f32 - 2.0))).collect();
    let mut b = Bench::new(level99(&[]), &around);
    b.status().set_mp(100);
    b.cast(cmd::THUNDER);
    b.run(120);
    let hits = b.hits();
    assert_eq!(hits.len(), 5, "one column, every enemy inside once");
    assert!(hits.iter().all(|h| h.damage == 81.0 && h.info.bad_status == "DISCHARGE" && h.info.bad_time == 5.0));
    let mut targets: Vec<u32> = hits.iter().map(|h| h.target.0).collect();
    targets.sort();
    assert_eq!(targets, vec![1, 2, 3, 4, 5]);

    let mut b = Bench::new(level99(&[]), &around);
    b.status().set_mp(100);
    b.cast(cmd::AERO);
    b.run(200);
    let hits = b.hits();
    assert_eq!(hits.len(), 5);
    assert!(hits.iter().all(|h| h.damage == 41.0 && h.attack_name == "MAG_AERO"));
    let kinds: Vec<ETresDamageKind> = hits.iter().map(|h| h.damage_kind).collect();
    assert_eq!(kinds.iter().filter(|k| **k == ETresDamageKind::SwirlUp).count(), 3, "{kinds:?}");
    assert_eq!(kinds.iter().filter(|k| **k == ETresDamageKind::Blow).count(), 2);
    assert_eq!(&kinds[..3], &[ETresDamageKind::SwirlUp; 3], "the FIRST three are swirled");
    assert_eq!(hits[0].info.effect_time, 2.0);
    // The residue: a jump pad at the column for 6 s.
    assert_eq!(b.sora.magic().pads.len(), 1);
    b.run(400);
    assert!(b.sora.magic().pads.is_empty());
}

/// T5 / part I 3: Water: the barrier around Sora (row MAG_WATER_BARRIER 67), then the bolt (61);
/// Sora is invincible from the shot to the end of the state.
#[test]
fn water_barrier_bolt_and_invincibility() {
    let mut b = Bench::new(level99(&[]), &[dummy(1, 120.0, 0.0), dummy(2, 900.0, 0.0)]);
    b.status().set_mp(100);
    let hp = b.sora.status().hp;
    b.cast(cmd::WATER);
    let mut invincible_steps = 0;
    for _ in 0..200 {
        let out = b.step(&[]);
        if out.state == StateId::Firemagic && b.sora.cast().is_some_and(|c| c.bullets > 0) {
            invincible_steps += 1;
            b.sora.take_hit(kh_core::states::HitOnPlayer::new(ETresDamageKind::Small, 30.0, TargetId(1), Vec3::new(120.0, 0.0, 0.0)));
        }
    }
    assert!(invincible_steps > 30);
    assert_eq!(b.sora.status().hp, hp, "no damage from the shot to the end of the state");
    assert!(b.sora.magic().hits_refused > 30);
    let hits = b.hits();
    let barrier: Vec<_> = hits.iter().filter(|h| h.attack_name == "MAG_WATER_BARRIER").collect();
    let bolt: Vec<_> = hits.iter().filter(|h| h.attack_name == "MAG_WATER").collect();
    assert_eq!((barrier.len(), barrier[0].damage, barrier[0].target), (1, 67.0, TargetId(1)));
    assert_eq!((bolt.len(), bolt[0].damage), (1, 61.0));
    // After the state a hit lands again.
    b.sora.take_hit(kh_core::states::HitOnPlayer::new(ETresDamageKind::Small, 30.0, TargetId(1), Vec3::new(120.0, 0.0, 0.0)));
    b.run(2);
    assert_eq!(b.sora.status().hp, hp - 30);
}

/// T2.1 - T2.5: MP after N casts, the charge at the overdraw, refusal during the charge, the
/// charge time 50 s and 45.45 s with one MP Haste.
#[test]
fn mp_rules_and_charge() {
    for (abilities, expected) in [(&[][..], 50.0f32), (&["MP_HASTE"][..], 50.0 / 1.1)] {
        let mut b = Bench::new(level99(abilities), &[dummy(1, 800.0, 0.0)]);
        b.status().set_mp(100);
        for n in 1..=10 {
            b.cast(cmd::FIRE);
            b.until_state(StateId::Firemagic, 5);
            b.until_state(StateId::Idle, 120);
            assert_eq!(b.sora.status().mp, 100 - 10 * n);
        }
        assert!(!b.sora.status().mp_charging, "landing exactly on 0 does not start the charge");
        // T2.2: one more is accepted, the bolt is spawned, the charge starts at the shot.
        let hits_before = b.hits().len();
        b.cast(cmd::FIRE);
        b.until_state(StateId::Firemagic, 5);
        let mut charge_started = None;
        for n in 0..120 {
            b.step(&[]);
            if charge_started.is_none() && b.sora.status().mp_charging {
                charge_started = Some(n);
            }
        }
        assert!(charge_started.is_some() && b.hits().len() == hits_before + 1);
        // T2.3: refused during the charge.
        let executed = b.sora.command().executed;
        b.cast(cmd::FIRE);
        b.run(3);
        assert_eq!(b.sora.command().executed, executed);
        assert_eq!(b.step(&[]).state, StateId::Idle);
        // T2.4 / T2.5: the charge's length (from the shot).
        let mut steps = 120 - charge_started.unwrap() + 4;
        while b.sora.status().mp_charging {
            b.step(&[]);
            steps += 1;
            assert!(steps < 4000);
        }
        assert!((seconds(steps) - expected).abs() < 0.1, "charge {} s, expected {expected}", seconds(steps));
        assert_eq!(b.sora.status().mp, b.sora.status().max_mp());
    }
}

/// T3.3 - T3.6: a cast during a cast: refused before the shot; a Fire follow-up starts at once,
/// a Blizzard one at TresEnableChange; a jump is never accepted.
#[test]
fn cast_gates_and_queueing() {
    let mut b = Bench::new(level99(&[]), &[dummy(1, 800.0, 0.0)]);
    b.status().set_mp(100);
    b.cast(cmd::FIRE);
    b.until_state(StateId::Firemagic, 5);
    b.run(6);
    // t = 0.1 s: before the shot.
    let executed = b.sora.command().executed;
    b.cast(cmd::FIRE);
    assert_eq!(b.sora.command().executed, executed, "refused: no bullet yet");
    // A jump press does nothing.
    b.tap(PadButton::JUMP);
    assert_eq!(b.step(&[]).state, StateId::Firemagic);
    // t = 0.35 s: accepted; the second Fire starts within a couple of ticks, not at 0.733 s.
    b.run(11);
    assert_eq!(b.sora.cast().unwrap().bullets, 1);
    b.cast(cmd::FIRE);
    assert_eq!(b.sora.command().executed, executed + 1);
    let mut started = 0;
    for n in 1..30 {
        b.step(&[]);
        if b.sora.cast().is_some_and(|c| c.combo_index == 1) {
            started = n;
            break;
        }
    }
    assert!((1..=3).contains(&started), "the Fire follow-up started after {started} ticks");
    // In the second cast: Blizzard at t = 0.35 s starts at TresEnableChange (0.733 s).
    b.run(21);
    b.cast(cmd::BLIZZARD);
    let mut started = 0;
    for n in 1..60 {
        b.step(&[]);
        if b.sora.cast().is_some_and(|c| c.cmd == cmd::BLIZZARD) {
            started = n;
            break;
        }
    }
    let at = 0.35 + seconds(started) + 2.0 * DT;
    assert!((at - 0.733).abs() < 0.06, "the Blizzard follow-up started at {at} s of the cast");
}

/// T6.1 / T6.6: Fire x4 chained: casts 1-3 `MagicFire` 67, cast 4 `MagicShoot` with
/// MAG_FIRE_FIN 81 (PARABOLA), invincible until its shot; a 5th press is refused; the command
/// cool-down after the finisher.
#[test]
fn magic_combo_and_finisher() {
    let mut b = Bench::new(level99(&[]), &[dummy(1, 800.0, 0.0)]);
    b.status().set_mp(100);
    b.cast(cmd::FIRE);
    b.until_state(StateId::Firemagic, 5);
    let mut clips = vec![b.step(&[]).anim.name];
    let hp = b.sora.status().hp;
    for index in 1..4 {
        // Press after the shot, once he stands again (the shot sets Falling for a few frames:
        // a press inside them is an air cast - `fire_magic::FALLING_AT_GROUND_SHOT`).
        while b.sora.cast().is_some_and(|c| c.bullets == 0) || !b.sora.output().on_floor {
            b.step(&[]);
        }
        b.cast(cmd::FIRE);
        for _ in 0..10 {
            let out = b.step(&[]);
            if b.sora.cast().is_some_and(|c| c.combo_index == index) {
                clips.push(out.anim.name);
                break;
            }
        }
    }
    assert_eq!(clips, vec![AnimId::MagicFire, AnimId::MagicFire, AnimId::MagicFire, AnimId::MagicShoot]);
    // The finisher is invincible from its first tick to its shot (0.7 s).
    b.sora.take_hit(kh_core::states::HitOnPlayer::new(ETresDamageKind::Small, 30.0, TargetId(1), Vec3::new(800.0, 0.0, 0.0)));
    b.run(3);
    assert_eq!((b.sora.status().hp, b.step(&[]).state), (hp, StateId::Firemagic));
    while b.sora.cast().is_some_and(|c| c.bullets == 0) {
        b.step(&[]);
    }
    // A 5th press is refused: the combo is over.
    let executed = b.sora.command().executed;
    b.cast(cmd::FIRE);
    assert_eq!(b.sora.command().executed, executed);
    b.until_state(StateId::Idle, 120);
    let hits = b.hits();
    assert_eq!(hits.iter().map(|h| h.damage).collect::<Vec<_>>(), vec![67.0, 67.0, 67.0, 81.0]);
    assert_eq!((hits[3].attack_name, hits[3].damage_kind), ("MAG_FIRE_FIN", ETresDamageKind::Parabola));
    // T6.6: MyCommandCoolDownTime (0.6 s) before the next cast is accepted.
    let executed = b.sora.command().executed;
    b.cast(cmd::FIRE);
    assert_eq!(b.sora.command().executed, executed, "refused inside the cool-down");
    b.run(40);
    b.cast(cmd::FIRE);
    assert_eq!(b.sora.command().executed, executed + 1);
}

/// T7.1, T7.3, T7.5: Curaga heals 160 + 67 = 227 and takes all MP (the charge starts); refused
/// during the charge; without Leaf Bracer a hit before the shot cancels it with no MP spent.
#[test]
fn cure() {
    let mut b = Bench::new(level99(&[]), &[]);
    b.status().set_mp(100);
    b.status().set_hp(1);
    b.cast(cmd::CURAGA);
    b.until_state(StateId::Firemagic, 5);
    assert_eq!(b.step(&[]).anim.name, AnimId::MagicCast);
    b.until_state(StateId::Idle, 120);
    assert_eq!(b.sora.status().hp, 1 + 227);
    assert_eq!((b.sora.status().mp, b.sora.status().mp_charging), (0, true));
    let executed = b.sora.command().executed;
    b.cast(cmd::CURE);
    assert_eq!(b.sora.command().executed, executed, "refused during MP charge");
    // Cure +147, Cura +187.
    for (kind, heal) in [(cmd::CURE, 147), (cmd::CURA, 187)] {
        let mut b = Bench::new(level99(&[]), &[]);
        b.status().set_hp(1);
        b.cast(kind);
        b.run(120);
        assert_eq!(b.sora.status().hp, 1 + heal);
    }
    // A hit before the Shot notify (0.3333 s) cancels: no heal, no MP spent.
    let mut b = Bench::new(level99(&[]), &[]);
    b.status().set_mp(100);
    b.status().set_hp(100);
    b.cast(cmd::CURAGA);
    b.until_state(StateId::Firemagic, 5);
    b.run(6);
    b.sora.take_hit(kh_core::states::HitOnPlayer::new(ETresDamageKind::Small, 30.0, TargetId(9), Vec3::new(200.0, 0.0, 0.0)));
    b.run(120);
    assert_eq!((b.sora.status().hp, b.sora.status().mp, b.sora.status().mp_charging), (70, 100, false));
    // With Leaf Bracer: no damage, no reaction from Begin to End.
    let mut b = Bench::new(level99(&["LEAF_VEIL"]), &[]);
    b.status().set_hp(100);
    b.cast(cmd::CURE);
    b.until_state(StateId::Firemagic, 5);
    b.run(6);
    b.sora.take_hit(kh_core::states::HitOnPlayer::new(ETresDamageKind::Small, 30.0, TargetId(9), Vec3::new(200.0, 0.0, 0.0)));
    b.run(120);
    assert_eq!(b.sora.status().hp, 100 + 147);
}

/// T5 modifiers: Fire Boost 81; Full MP Blast 101.
#[test]
fn damage_modifiers() {
    let mut b = Bench::new(level99(&["FIRE_UP"]), &[dummy(1, 600.0, 0.0)]);
    b.status().set_mp(100);
    b.cast(cmd::FIRE);
    b.run(120);
    assert_eq!(b.hits()[0].damage, 81.0);
    let mut b = Bench::new(level99(&[]), &[dummy(1, 600.0, 0.0)]);
    b.status().set_ability(ETresAbilityKind::FullmpBurst, 1, 1);
    let full = b.sora.status().max_mp();
    b.status().set_mp(full);
    b.cast(cmd::FIRE);
    b.run(120);
    assert_eq!(b.hits()[0].damage, 101.0);
}

/// T6b.4: a grand-magic command: no MP, the `_S` row (Fira: power 3.0 -> 201), invincible until
/// the shot, the command lock for 0.1 s.
#[test]
fn grand_magic_command() {
    let mut b = Bench::new(level99(&[]), &[dummy(1, 800.0, 0.0)]);
    b.status().set_mp(100);
    b.cast(cmd::S_FIRA);
    assert!(b.sora.command().lock_timer > 0.0 && b.sora.command().reaction_block > 0.0);
    b.until_state(StateId::Firemagic, 5);
    assert_eq!(b.step(&[]).anim.name, AnimId::MagicShoot);
    b.until_state(StateId::Idle, 150);
    assert_eq!(b.sora.status().mp, 100, "no MP cost");
    let hit = b.hits()[0];
    assert_eq!((hit.attack_name, hit.damage), ("MAG_FIRA_S", 201.0));
    // The offer table of part I 7.2: one tier above the highest tier cast.
    use kh_core::magic::{grand, Element};
    assert_eq!((grand::offer_of(Element::Fire, cmd::FIRE), grand::offer_of(Element::Fire, cmd::FIRA), grand::offer_of(Element::Fire, cmd::FIRAGA)), (110, 111, 112));
    assert_eq!(grand::lifetime(b.sora.status()), 20.0);
}

// ------------------------------------------------------------------------------------ items

/// T8.1, T8.3, T8.8 + part J 7: a Potion: the state lasts 0.8333 s, the slot is consumed at
/// 0.5 s, HP +80 at 1.0 s; an empty pouch refuses; auto-reload slots are refilled from the
/// stock when the game leaves a pause.
#[test]
fn items_and_the_pouch() {
    let mut b = Bench::new(level99(&[]), &[]);
    b.status().set_hp(1);
    b.cast(cmd::POTION);
    assert_eq!(b.until_state(StateId::UseItem, 5), 1);
    assert_eq!(b.sora.magic().pouch.slot_count(), 8);
    assert_eq!(b.sora.magic().pouch.count(ItemKind::Potion), 2);
    let (mut consumed_at, mut healed_at, mut ended_at) = (0, 0, 0);
    for n in 1..90 {
        let out = b.step(&[]);
        if consumed_at == 0 && b.sora.magic().pouch.count(ItemKind::Potion) == 1 {
            consumed_at = n;
        }
        if healed_at == 0 && b.sora.status().hp == 81 {
            healed_at = n;
        }
        if ended_at == 0 && out.state != StateId::UseItem {
            ended_at = n;
        }
    }
    assert!((seconds(consumed_at) - 0.5).abs() <= 2.0 * DT, "consumed at {}", seconds(consumed_at));
    assert!((seconds(healed_at) - 1.0).abs() <= 3.0 * DT, "healed at {}", seconds(healed_at));
    assert!((seconds(ended_at) - 0.8333).abs() <= 3.0 * DT, "ended at {}", seconds(ended_at));
    // Hi-Potion +160; Ether +100 MP.
    b.cast(cmd::HIPOTION);
    b.run(90);
    assert_eq!(b.sora.status().hp, 81 + 160);
    b.status().set_mp(0);
    b.cast(cmd::ETHER);
    b.run(90);
    assert_eq!(b.sora.status().mp, 100);
    // The second Potion, then the pouch has none: refused.
    b.cast(cmd::POTION);
    b.run(90);
    let executed = b.sora.command().executed;
    b.cast(cmd::POTION);
    b.run(3);
    assert_eq!((b.sora.command().executed, b.sora.magic().pouch.count(ItemKind::Potion)), (executed, 0));
    // The refill: used auto-reload slots take one each from the stock at an unpause.
    assert_eq!(b.sora.on_unpause(), 0, "nothing in the stock");
    b.sora.magic_mut().pouch.stock_add(ItemKind::Potion, 5);
    assert_eq!(b.sora.on_unpause(), 2);
    assert_eq!((b.sora.magic().pouch.count(ItemKind::Potion), b.sora.magic().pouch.stock_of(ItemKind::Potion)), (2, 3));
    // T8.7: in the air: `ItemAirUse`.
    b.tap(PadButton::JUMP);
    b.run(20);
    b.cast(cmd::POTION);
    b.until_state(StateId::UseItem, 5);
    assert_eq!(b.step(&[]).anim.name, AnimId::ItemAirUse);
}

// ------------------------------------------------------------------------------------ the menu

/// T11.1 - T11.4 through the PAD: the Magic list (six entries in learn order at the top tier),
/// D-pad cursor with wrap, confirm executes on the press frame and resets both cursors, cancel
/// and left close, MP charge greys the rows, L1 + face buttons run the shortcut slots, L1 +
/// D-pad changes the page with wrap.
#[test]
fn command_menu_and_shortcuts_from_the_pad() {
    let mut b = Bench::new(level99(&[]), &[dummy(1, 800.0, 0.0)]);
    b.status().set_mp(100);
    b.run(1);
    let ui = b.host.ui.clone().unwrap().command;
    assert_eq!((ui.state, ui.main_cursor, ui.main[0].name, ui.main[1].enabled, ui.main[2].enabled, ui.link_none), (MenuState::MainSel, 0, "Attack", true, true, true));
    assert_eq!(ui.lists[1].iter().map(|r| r.name).collect::<Vec<_>>(), vec!["Firaga", "Waterga", "Curaga", "Blizzaga", "Thundaga", "Aeroga"]);
    assert_eq!(ui.lists[2].iter().map(|r| (r.name, r.quantity)).collect::<Vec<_>>(), vec![("Potion", 2), ("Hi-Potion", 2), ("Ether", 2)]);
    // The default test shortcut pages.
    assert_eq!(ui.shortcut.iter().map(|r| r.name).collect::<Vec<_>>(), vec!["Firaga", "Blizzaga", "Thundaga", "Curaga"]);
    // D-pad up from Attack wraps to Items (three slots without Link); down twice: Magic.
    b.tap(PadButton::DPadUp);
    assert_eq!(b.sora.command().main_cursor, 2);
    b.tap(PadButton::DPadDown);
    b.tap(PadButton::DPadDown);
    assert_eq!(b.sora.command().main_cursor, 1);
    // Confirm opens the Magic list; D-pad up wraps to the last row; down returns.
    b.tap(PadButton::ATTACK);
    assert_eq!((b.sora.command().state, b.sora.command().list), (MenuState::SubSel, 1));
    b.tap(PadButton::DPadUp);
    assert_eq!(b.sora.command().sub_cursor, 5);
    b.tap(PadButton::DPadDown);
    assert_eq!(b.sora.command().sub_cursor, 0);
    // Cancel (the jump button) closes it without a jump: (DON-UI) the user's rule for all menus, back one level: the main
    // list with the cursor kept on Magic (KH3 returned it to Attack)
    let out = b.step(&[PadButton::JUMP]);
    b.run(3);
    assert_eq!((b.sora.command().state, b.sora.command().main_cursor, out.state, b.step(&[]).state), (MenuState::MainSel, 1, StateId::Idle, StateId::Idle));
    // Open with D-pad right; left closes and keeps the main cursor.
    b.tap(PadButton::DPadRight);
    assert_eq!(b.sora.command().state, MenuState::SubSel);
    b.tap(PadButton::DPadLeft);
    assert_eq!((b.sora.command().state, b.sora.command().main_cursor), (MenuState::MainSel, 1));
    // Confirm a spell: executed on the press frame; both cursors reset.
    b.tap(PadButton::ATTACK);
    b.tap(PadButton::DPadDown);
    b.tap(PadButton::DPadDown);
    b.tap(PadButton::DPadDown);
    let executed = b.sora.command().executed;
    b.step(&[PadButton::ATTACK]);
    assert_eq!((b.sora.command().executed, b.sora.command().last_executed, b.sora.command().pending), (executed + 1, cmd::BLIZZAGA, 0));
    assert_eq!((b.sora.command().state, b.sora.command().main_cursor, b.sora.command().sub_cursor), (MenuState::MainSel, 0, 0));
    let events = b.host.ui.clone().unwrap().command.events;
    assert!(events.contains(&MenuEvent::Executed(cmd::BLIZZAGA)));
    assert_eq!(b.step(&[]).state, StateId::Firemagic);
    b.until_state(StateId::Idle, 150);
    assert_eq!(b.sora.status().mp, 100 - 24);
    // Shortcuts: L1 + Circle (slot 0 = Firaga); the jump button does not jump with L1 held.
    b.step(&[PadButton::LeftShoulder]);
    assert_eq!(b.sora.command().state, MenuState::Shortcut);
    b.step(&[PadButton::LeftShoulder, PadButton::FaceRight]);
    assert_eq!(b.sora.command().last_executed, cmd::FIRAGA);
    let out = b.step(&[PadButton::LeftShoulder]);
    assert_eq!(out.state, StateId::Firemagic);
    // L1 + D-pad down: page B (Waterga, Aeroga, Potion, Ether); up twice wraps to C; up: B.
    b.step(&[PadButton::LeftShoulder, PadButton::DPadDown]);
    b.step(&[PadButton::LeftShoulder]);
    assert_eq!(b.host.ui.clone().unwrap().command.shortcut.iter().map(|r| r.name).collect::<Vec<_>>(), vec!["Waterga", "Aeroga", "Potion", "Ether"]);
    b.step(&[PadButton::LeftShoulder, PadButton::DPadUp]);
    b.step(&[PadButton::LeftShoulder]);
    b.step(&[PadButton::LeftShoulder, PadButton::DPadUp]);
    assert_eq!(b.sora.command().shortcut_page, 2);
    // An empty slot does nothing.
    let executed = b.sora.command().executed;
    b.step(&[PadButton::LeftShoulder]);
    b.step(&[PadButton::LeftShoulder, PadButton::FaceTop]);
    assert_eq!(b.sora.command().executed, executed);
    b.step(&[]);
    assert_eq!(b.sora.command().state, MenuState::MainSel);
    // T11.2: during MP charge the magic rows and the Magic slot are greyed; confirm is refused.
    b.until_state(StateId::Idle, 200);
    b.status().set_mp(-1);
    b.run(2);
    assert!(b.sora.status().mp_charging);
    let ui = b.host.ui.clone().unwrap().command;
    assert!(ui.mp_charging && !ui.main[1].enabled && ui.lists[1].iter().all(|r| !r.enabled));
    b.tap(PadButton::DPadDown);
    b.step(&[PadButton::ATTACK]);
    assert!(b.host.ui.clone().unwrap().command.events.contains(&MenuEvent::Refused));
    assert_eq!(b.sora.command().state, MenuState::MainSel);
    // mpShort recolours a row that is still castable: Firaga (20) at MP 15.
    let mut b = Bench::new(level99(&[]), &[]);
    b.status().set_mp(15);
    b.run(1);
    let row = b.host.ui.clone().unwrap().command.lists[1][0];
    assert_eq!((row.name, row.mp_short, row.enabled), ("Firaga", true, true));
    // The Attack row executes the attack command through the menu.
    let mut b = Bench::new(level99(&[]), &[dummy(1, 150.0, 0.0)]);
    b.tap(PadButton::ATTACK);
    assert_eq!((b.sora.command().last_executed, b.step(&[]).state), (cmd::ATTACK, StateId::Attack));
}

// ------------------------------------------------------------------------------------ shotlock

fn locks_of(b: &Bench) -> i32 {
    b.sora.aim().total
}

/// T9.1, T9.2, T9.3, T9.6: a tap is no aim mode; a hold opens it after 0.2667 s with the
/// player and the world at 0.4; the first lock >= 0.1 s later, then one per 0.0933 s of real
/// time; after 1.25 s of aiming a release leaves focus 75; a cancel costs nothing.
#[test]
fn shotlock_aim_mode() {
    let mut b = Bench::new(level99(&[]), &[dummy(1, 800.0, 0.0), dummy(2, 800.0, 150.0)]);
    assert_eq!(b.sora.status().focus, 100);
    // A tap (released before 0.2667 s).
    b.hold(&[PadButton::RightShoulder], 10);
    b.run(2);
    assert_eq!(b.sora.aim().mode, 0);
    // (The tap locked on: release it again so that the aim mode starts from the soft target.)
    b.hold(&[PadButton::RightShoulder], 5);
    b.run(30);
    // The hold.
    let mut opened = 0;
    for n in 1..40 {
        b.step(&[PadButton::RightShoulder]);
        if b.sora.aim().mode == 1 {
            opened = n;
            break;
        }
    }
    assert!((seconds(opened) - 0.2667).abs() <= 2.0 * DT, "opened after {}", seconds(opened));
    assert_eq!((b.sora.time_dilation(1), b.sora.time_dilation(2), b.sora.time_dilation(5), b.sora.time_dilation(9)), (0.4, 0.4, 1.0, 1.0));
    assert_eq!(b.host.ui.clone().unwrap().command.state, MenuState::ShootFlow);
    // The first lock comes after the 0.1 s start delay; then the cadence 2.24 / 24 s.
    let mut times = Vec::new();
    let mut last = 0;
    for n in 1..=200 {
        if b.sora.magic().aim.aimed >= 1.2 {
            break;
        }
        b.step(&[PadButton::RightShoulder]);
        if locks_of(&b) > last {
            last = locks_of(&b);
            times.push(seconds(n));
        }
    }
    assert!(times[0] >= 0.1 && times[0] < 0.1 + 3.0 * DT, "first lock at {}", times[0]);
    let cadence = (times[times.len() - 1] - times[0]) / (times.len() - 1) as f32;
    assert!((cadence - 0.0933).abs() < 0.012, "cadence {cadence}");
    // 1.25 s of aiming after the start delay (the last step ends just inside it: at exactly
    // 1.25 s `trunc(3.75 / 0.05)` sits on the float boundary between 74 and 75).
    let n = locks_of(&b);
    assert!((12..=14).contains(&n), "{n} locks after 1.25 s");
    // Round robin over the two dummies in the reticle.
    let view = b.sora.aim();
    assert_eq!(view.locks.len(), 2);
    assert!((view.locks[0].1 - view.locks[1].1).abs() <= 1);
    // Fire: focus = int((5.0 - 1.25) / 0.05) = 75.
    b.step(&[PadButton::RightShoulder, PadButton::ATTACK]);
    assert_eq!(b.sora.status().focus, 75);
    assert_eq!((b.sora.time_dilation(1), b.sora.time_dilation(2)), (1.0, 0.4), "Sora back to full speed, the world stays slowed");
    assert_eq!(b.step(&[PadButton::RightShoulder]).state, StateId::Shootflow);

    // T9.6: a cancel (R1 released without firing) costs no focus.
    let mut b = Bench::new(level99(&[]), &[dummy(1, 800.0, 0.0)]);
    b.hold(&[PadButton::RightShoulder], 60);
    assert!(b.sora.aim().mode == 1 && locks_of(&b) > 0);
    b.run(2);
    assert_eq!((b.sora.aim().mode, b.sora.status().focus, b.sora.time_dilation(1)), (0, 100, 1.0));
    // T9.5: a dummy outside the 150 px circle is never locked; the attack key does nothing.
    let mut b = Bench::new(level99(&[]), &[dummy(1, 800.0, 700.0)]);
    b.hold(&[PadButton::RightShoulder], 90);
    assert_eq!((b.sora.aim().mode, locks_of(&b)), (1, 0));
    b.step(&[PadButton::RightShoulder, PadButton::ATTACK]);
    assert_eq!(b.sora.aim().mode, 1);
    // T9.7: a dummy behind level geometry is not locked.
    let mut b = Bench::new(level99(&[]), &[dummy(1, 800.0, 0.0)]);
    b.world = TestWorld::floor().with_box(Vec3::new(380.0, -200.0, 0.0), Vec3::new(420.0, 200.0, 400.0));
    b.hold(&[PadButton::RightShoulder], 90);
    assert_eq!((b.sora.aim().mode, locks_of(&b)), (1, 0));
}

/// T9.4: King of Hearts: the timeline from Begin, 7 projectiles per tick, every hit 54
/// (SF_KCSHOT), targets `locked[i mod lockedTargets]`, Sora invincible throughout, the state
/// ends at about 4.0 s in the air (lifted 500 cm).
#[test]
fn king_of_hearts() {
    let mut b = Bench::new(level99(&[]), &[dummy(1, 800.0, 0.0), dummy(2, 800.0, 150.0)]);
    b.hold(&[PadButton::RightShoulder], 17 + 6 + 62);
    let n = locks_of(&b) as usize;
    assert!((9..=12).contains(&n), "{n} locks");
    b.step(&[PadButton::RightShoulder, PadButton::ATTACK]);
    assert_eq!(b.sora.aim().kind, 1);
    let begin = b.until_state(StateId::Shootflow, 3);
    let hp = b.sora.status().hp;
    let (mut lifted_at, mut first_spawn, mut all_out, mut first_hit, mut ended) = (0, 0, 0, 0, 0);
    let mut per_tick = Vec::new();
    let mut before = 0;
    for k in 1..400 {
        let out = b.step(&[]);
        if k % 20 == 0 {
            b.sora.take_hit(kh_core::states::HitOnPlayer::new(ETresDamageKind::Small, 30.0, TargetId(1), Vec3::new(800.0, 0.0, 0.0)));
        }
        if lifted_at == 0 && out.position.z > 400.0 {
            lifted_at = k;
        }
        let count = b.sora.magic().projectile_views().count();
        if count > before && first_hit == 0 {
            per_tick.push(count - before);
            if first_spawn == 0 {
                first_spawn = k;
            }
            if count == n {
                all_out = k;
            }
        }
        before = count;
        if first_hit == 0 && !b.hits().is_empty() {
            first_hit = k;
        }
        if out.state != StateId::Shootflow {
            ended = k;
            break;
        }
    }
    let _ = begin;
    assert!(lifted_at <= 3, "lifted on tick {lifted_at}");
    assert!((seconds(first_spawn) - 2.5).abs() < 0.08, "set-up at {}", seconds(first_spawn));
    assert_eq!(per_tick, if n > 7 { vec![7, n - 7] } else { vec![n] });
    assert_eq!(all_out, first_spawn + 1);
    assert!(seconds(first_hit) > 3.07, "launch about 3.07 s; first hit at {}", seconds(first_hit));
    assert!((seconds(ended) - 4.0).abs() < 0.12, "ended at {}", seconds(ended));
    assert_eq!(b.sora.status().hp, hp, "invincible throughout");
    b.run(240);
    let hits = b.hits();
    assert_eq!(hits.len(), n, "one hit per projectile");
    assert!(hits.iter().all(|h| h.damage == 54.0 && h.attack_name == "SF_KCSHOT" && h.flags == 0xFD | 1 << 8));
    let on = |id: u32| hits.iter().filter(|h| h.target == TargetId(id)).count();
    assert_eq!((on(1).max(on(2)), on(1).min(on(2))), (n.div_ceil(2), n / 2));
    // The aim mode is over and every type is back at full speed.
    assert_eq!((b.sora.aim().mode, b.sora.time_dilation(2)), (0, 1.0));
}

/// T9.8: 24 locks -> MAX, the kind becomes Ragnarok; 24 projectiles in one volley, 61 each
/// (SF_KCRAGNAROK); the consumed focus is frozen while MAX holds.
#[test]
fn ragnarok() {
    let mut b = Bench::new(level99(&[]), &[dummy(1, 900.0, 0.0)]);
    b.hold(&[PadButton::RightShoulder], 17 + 6);
    let mut maxed = 0;
    for k in 1..200 {
        b.step(&[PadButton::RightShoulder]);
        if b.sora.aim().is_max {
            maxed = k;
            break;
        }
    }
    // 24 locks: (24 - 1) x 0.0933 = 2.147 s after the first.
    assert!((seconds(maxed) - 2.147).abs() < 0.2, "MAX after {}", seconds(maxed));
    assert_eq!((b.sora.aim().total, b.sora.aim().kind), (24, 2));
    // Waiting at MAX costs nothing more.
    b.hold(&[PadButton::RightShoulder], 60);
    b.step(&[PadButton::RightShoulder, PadButton::ATTACK]);
    let focus = b.sora.status().focus;
    assert!((52..=58).contains(&focus), "focus {focus} (about 100 - 20 x 2.2 s)");
    b.until_state(StateId::Shootflow, 3);
    let mut ended = 0;
    for k in 1..400 {
        if b.step(&[]).state != StateId::Shootflow {
            ended = k;
            break;
        }
    }
    assert!((seconds(ended) - 4.133).abs() < 0.12, "ended at {}", seconds(ended));
    b.run(400);
    let hits = b.hits();
    assert_eq!(hits.len(), 24);
    assert!(hits.iter().all(|h| h.damage == 61.0 && h.attack_name == "SF_KCRAGNAROK" && h.damage_kind == ETresDamageKind::Toss));
}

/// T9.9: Focus Syphon: focus 0 with FOCUS_ASPIR: the aim mode opens without slow motion, locks
/// by distance, and the cast returns max(1, trunc(1.4 x locks)) focus without damage.
#[test]
fn focus_syphon() {
    let mut b = Bench::new(level99(&["FOCUS_ASPIR"]), &[dummy(1, 500.0, 0.0)]);
    b.status().set_focus(0);
    assert!(b.sora.status().focus_empty);
    b.hold(&[PadButton::RightShoulder], 17 + 6);
    assert_eq!((b.sora.aim().mode, b.sora.aim().kind, b.sora.time_dilation(1)), (4, 36, 1.0));
    b.hold(&[PadButton::RightShoulder], 200);
    assert_eq!((b.sora.aim().total, b.sora.aim().is_max), (18, true));
    b.step(&[PadButton::RightShoulder, PadButton::ATTACK]);
    b.until_state(StateId::Shootflow, 3);
    b.run(150);
    assert_eq!(b.sora.status().focus, 25);
    assert!(b.hits().is_empty() && b.sora.status().focus_empty, "no damage; the flag stays until the gauge is full");
    assert!(ETresAbilityKind::FocusAspir.value() == 46);
}

/// T10.1, T10.3, T10.5: airstep to a dummy 2000 cm away: state 42, no focus cost, launch speed
/// 6000 cm/s, 3000 inside the last 600 cm, arrival at distance < 200 with `AthleticSlideEnd`,
/// invincible; and to a level point.
#[test]
fn airstep() {
    let mut b = Bench::new(level99(&[]), &[dummy(1, 2000.0, 0.0)]);
    b.hold(&[PadButton::RightShoulder], 40);
    assert!(b.sora.aim().can_airstep);
    assert_eq!(b.sora.aim().airstep, vec![(TargetId(1), 0)]);
    b.step(&[PadButton::RightShoulder, PadButton::FaceLeft]);
    assert_eq!((b.sora.aim().mode, b.sora.status().focus), (3, 100));
    b.until_state(StateId::Athleticflow, 3);
    let hp = b.sora.status().hp;
    let (mut top_speed, mut arrived, mut ended) = (0.0f32, 0, 0);
    let mut near_speed = 0.0f32;
    for k in 1..300 {
        let out = b.step(&[]);
        b.sora.take_hit(kh_core::states::HitOnPlayer::new(ETresDamageKind::Small, 30.0, TargetId(1), Vec3::new(2000.0, 0.0, 0.0)));
        let speed = out.velocity.length();
        top_speed = top_speed.max(speed);
        if out.position.x > 1500.0 && arrived == 0 {
            near_speed = near_speed.max(speed);
        }
        if arrived == 0 && out.anim.name == AnimId::AthleticSlideEnd {
            arrived = k;
            let d = (Vec3::new(2000.0, 0.0, 90.0) - (out.position + Vec3::Z * 100.0)).length();
            assert!(d < 200.0 + 6000.0 * DT, "arrived at distance {d}");
        }
        if out.state != StateId::Athleticflow {
            ended = k;
            break;
        }
    }
    assert!((top_speed - 6000.0).abs() < 0.01 && (near_speed - 3000.0).abs() < 0.01, "{top_speed} {near_speed}");
    assert!(arrived > 0 && ended > arrived);
    assert_eq!(b.sora.status().hp, hp, "invincible for the whole state");
    b.run(5);
    assert_eq!((b.sora.aim().mode, b.sora.time_dilation(MASK_PLAYER.trailing_zeros()), b.sora.time_dilation(2)), (0, 1.0, 1.0));
    // A level point (kind WALL) as the last hop: the snap, then `SlideEnd`.
    let mut b = Bench::new(level99(&[]), &[]);
    b.sora.magic_mut().airstep_points.push(kh_core::magic::AirstepPoint::wall(7, Vec3::new(1500.0, 0.0, 300.0)));
    b.view = Some(View::from_camera(Vec3::new(-300.0, 0.0, 150.0), 0.0, 9.0, 90.0, 1280, 720));
    b.hold(&[PadButton::RightShoulder], 40);
    assert_eq!(b.sora.aim().airstep, vec![(TargetId::NONE, 7)]);
    b.step(&[PadButton::RightShoulder, PadButton::FaceLeft]);
    b.until_state(StateId::Athleticflow, 3);
    let mut reached = false;
    for _ in 0..200 {
        let out = b.step(&[]);
        if out.anim.name == AnimId::SlideEnd {
            reached = (out.position - Vec3::new(1500.0, 0.0, 300.0)).length() < 5.0;
        }
        if out.state != StateId::Athleticflow {
            break;
        }
    }
    assert!(reached);
    // T10.0: without the unlock no airstep target is ever offered.
    let mut b = Bench::new(level99(&[]), &[dummy(1, 2000.0, 0.0)]);
    b.sora.magic_mut().airstep_unlocked = false;
    b.hold(&[PadButton::RightShoulder], 40);
    assert!(!b.sora.aim().can_airstep);
    let _ = EventKind::AimMode { open: true, reason: 0 };
}

/// (IMPL_ER42, port, user request "shotlock should aim at softlock target") The aim's entry remembers the soft target for
/// the hosts' aim camera when no lock is on (`saved_soft`); with a lock on, the lock (`saved_lock`, KH3's 0x9A4) and no soft.
#[test]
fn shotlock_entry_remembers_the_soft_target() {
    let mut b = Bench::new(level99(&[]), &[dummy(1, 800.0, 0.0)]);
    b.run(10);
    let lock = b.sora.combat().lockon;
    assert!(!lock.locked && lock.current == TargetId(1), "soft target before the aim: {:?} locked {}", lock.current, lock.locked);
    b.hold(&[PadButton::RightShoulder], 30);
    assert_eq!(b.sora.aim().mode, 1);
    assert_eq!((b.sora.magic().aim.saved_lock, b.sora.magic().aim.saved_soft), (TargetId::NONE, TargetId(1)));
    b.run(30);
    assert_eq!((b.sora.aim().mode, b.sora.magic().aim.saved_soft), (0, TargetId::NONE), "cleared at the exit");
    // A tap locks on; the hold then remembers the lock, not a soft target.
    b.hold(&[PadButton::RightShoulder], 5);
    b.run(30);
    assert!(b.sora.combat().lockon.locked);
    b.hold(&[PadButton::RightShoulder], 30);
    assert_eq!(b.sora.aim().mode, 1);
    assert_eq!((b.sora.magic().aim.saved_lock, b.sora.magic().aim.saved_soft), (TargetId(1), TargetId::NONE));
}
