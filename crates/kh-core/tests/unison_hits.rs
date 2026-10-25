//! (party stream D6 / D6b) The team finishers hit what they hit in KH3: Sora's level-99 core plays
//! DoUThunder / DoUFire / DoUBlizzard next to Donald, GooFSpin / GooFRocket next to Goofy, and the
//! shots (sheet sora-combat-shots; attack-state slots 137 0x14134CCE0 / 138 0x14134C710) land on
//! bodies beside, in front of, behind and around him as their blueprints' shapes say:
//! TresProj_UnisonThunder CONVEX tres_colltest01 at +150 (radius 200 -> 800, 1200 tall), UnisonFire
//! SPHERE at +150 to 800, UnisonBlizzard SPHERE at the actor to 800 then UNISON_BLIZZARD_END,
//! FusionSpin SPHERE 50 -> 400 at +150 carried by Goofy; Fusion Rocket is the clip's SphereM window.
//! Before D6 Unison Thunder was a 50 cm sphere 2.4 m above Sora's feet and hit nothing standing.

use std::collections::BTreeMap;

use kh_core::host::audio::NullAudio;
use kh_core::host::effects::{EffectHandle, EffectSpawn, Effects};
use kh_core::host::{Attach, Friend, FriendLinkRequest, FriendLinkView, Host, HitRecord, HitResult, HurtShape, NullUi, Targets};
use kh_core::status::LoadoutValue;
use kh_core::test_world::TestWorld;
use kh_core::{Input, Loadout, PadButton, PadButtons, PadState, Pose, Sora, Status, TargetFlags, TargetId, TargetInfo, Tuning, Vec3, SORA_PARAMS};

const DT: f32 = 1.0 / 60.0;

fn level99() -> Status {
    let path = format!("{}/../../sheets/port/sora-loadout-99.json", env!("CARGO_MANIFEST_DIR"));
    let json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let mut loadout = Loadout::new("sora-loadout-99");
    for row in json["rows"].as_array().unwrap() {
        let v = match &row[1] {
            serde_json::Value::Number(n) => LoadoutValue::Number(n.as_f64().unwrap()),
            serde_json::Value::String(s) => LoadoutValue::Text(s.clone()),
            serde_json::Value::Array(a) => LoadoutValue::List(a.iter().map(|i| i.as_str().unwrap().to_string()).collect()),
            serde_json::Value::Object(m) => LoadoutValue::Map(m.iter().map(|(k, v)| (k.clone(), v.as_f64().unwrap())).collect()),
            _ => continue,
        };
        let v = match (row[0].as_str().unwrap(), v) {
            ("abilities.equipped", LoadoutValue::List(mut l)) => {
                l.extend(["UNISON_FIRE", "UNISON_BLIZZARD", "UNISON_THUNDER", "FUSION_SPIN", "FUSION_ROCKET"].map(String::from));
                LoadoutValue::List(l)
            }
            (_, v) => v,
        };
        loadout.set(row[0].as_str().unwrap(), &v).unwrap();
    }
    loadout.resolve().unwrap();
    Status::from_loadout(&loadout)
}

/// Standing dummies (capsule radius 40, 180 tall on the floor) and Donald / Goofy beside Sora;
/// Goofy may walk (`goofy_speed` cm/s along +X).
struct Field {
    dummies: Vec<(TargetId, Vec3)>,
    hits: BTreeMap<u32, Vec<&'static str>>,
    points: Vec<(&'static str, Vec3)>,
    goofy: Vec3,
    goofy_speed: f32,
}

impl Field {
    fn info(&self, id: TargetId, at: Vec3) -> TargetInfo {
        TargetInfo { id, position: at, center: at + Vec3::Z * 90.0, velocity: Vec3::ZERO, yaw: 180.0, radius: 40.0, half_height: 90.0, flags: TargetFlags(TargetFlags::ALIVE | TargetFlags::HITTABLE | TargetFlags::LOCKABLE | TargetFlags::ENEMY), hp: 1.0e9, hp_max: 1.0e9 }
    }
}

impl Targets for Field {
    fn enumerate(&self, out: &mut Vec<TargetInfo>) {
        out.extend(self.dummies.iter().map(|&(id, at)| self.info(id, at)));
    }
    fn info(&self, id: TargetId) -> Option<TargetInfo> {
        self.dummies.iter().find(|d| d.0 == id).map(|&(id, at)| Field::info(self, id, at))
    }
    fn hurt_shapes(&self, id: TargetId, out: &mut Vec<HurtShape>) {
        if let Some(&(_, at)) = self.dummies.iter().find(|d| d.0 == id) {
            out.push(HurtShape { part: 0, a: at + Vec3::Z * 40.0, b: at + Vec3::Z * 140.0, radius: 40.0 });
        }
    }
    fn apply_hit(&mut self, hit: &HitRecord) -> HitResult {
        self.hits.entry(hit.target.0).or_default().push(hit.attack_name);
        self.points.push((hit.attack_name, hit.position));
        HitResult::default()
    }
    fn friends(&self, out: &mut Vec<Friend>) {
        for (uid, at) in [(11u8, Vec3::new(0.0, 150.0, 0.0)), (12, self.goofy)] {
            out.push(Friend { uid, position: at, yaw: 0.0, ko: false, combi_time: 1, combi_up: 0, offer: None, link: FriendLinkView::default() });
        }
    }
    fn friend_link(&mut self, _req: FriendLinkRequest) {}
}

/// The effects spawned at a world point (name, point).
#[derive(Default)]
struct Fx(Vec<(&'static str, Vec3)>);

impl Effects for Fx {
    fn spawn(&mut self, _handle: EffectHandle, spawn: &EffectSpawn) {
        if let Attach::World(p) = spawn.at {
            self.0.push((spawn.name, p));
        }
    }
    fn stop(&mut self, _handle: EffectHandle, _immediate: bool) {}
}

const FRONT: u32 = 1;
const BESIDE: u32 = 2;
const BEHIND: u32 = 3;
const DIAGONAL: u32 = 4;
const FAR: u32 = 5;

struct Run {
    hits: BTreeMap<u32, Vec<&'static str>>,
    /// (row, hit point) of every hit.
    points: Vec<(&'static str, Vec3)>,
    effects: Vec<(&'static str, Vec3)>,
    history: Vec<u8>,
}

/// Sora at the origin facing +X; one dummy right in front (the combo's target), one beside him
/// (1 m to the side), one behind, one 5 m away diagonally, one 15 m away. Donald at (0, 150),
/// Goofy at (0, -150) (walking along +X at `goofy_speed`). The unison kind is latched as the roll
/// would, then one A press plays it.
fn run_with(kind: u8, goofy_speed: f32) -> Run {
    let origin = Vec3::new(0.0, 0.0, Sora::origin_height_on_floor(&SORA_PARAMS));
    let mut sora = Sora::new(SORA_PARAMS, Pose { position: origin, yaw: 0.0 }).with_tuning(Tuning::GAME_EXACT).with_status(level99());
    let floor = |x: f32, y: f32| Vec3::new(x, y, 0.0);
    let mut field = Field { dummies: vec![(TargetId(FRONT), floor(160.0, 0.0)), (TargetId(BESIDE), floor(0.0, 100.0)), (TargetId(BEHIND), floor(-250.0, 0.0)), (TargetId(DIAGONAL), floor(350.0, 350.0)), (TargetId(FAR), floor(1500.0, 0.0))], hits: BTreeMap::new(), points: Vec::new(), goofy: Vec3::new(0.0, -150.0, 0.0), goofy_speed: 0.0 };
    let world = TestWorld::floor();
    let mut fx = Fx::default();
    let step = |sora: &mut Sora, field: &mut Field, fx: &mut Fx, held: &[PadButton]| {
        let set = |l: &[PadButton]| l.iter().fold(PadButtons::NONE, |a, b| a.with(*b));
        let input = Input::default().with_pad(PadState::from_held(set(held), set(&[])));
        let (mut audio, mut ui) = (NullAudio, NullUi);
        let mut host = Host { world: &world, targets: field, audio: &mut audio, effects: fx, ui: &mut ui };
        sora.step_hosted(DT, &input, &mut host);
        field.goofy.x += field.goofy_speed * DT;
    };
    for _ in 0..5 {
        step(&mut sora, &mut field, &mut fx, &[]);
    }
    // The latch the roll leaves (FriendState::roll_leader_unison): the next attack plays it.
    (sora.combat_mut().friend.unison, sora.combat_mut().friend.leader_unison) = (kind, 0.5);
    for i in 0..150 {
        if i == 2 {
            field.goofy_speed = goofy_speed;
        }
        step(&mut sora, &mut field, &mut fx, if i < 2 { &[PadButton::ATTACK] } else { &[] });
    }
    Run { hits: field.hits, points: field.points, effects: fx.0, history: sora.combat().history.clone() }
}

fn run(kind: u8) -> Run {
    run_with(kind, 0.0)
}

fn landed(hits: &BTreeMap<u32, Vec<&'static str>>, id: u32, row: &str) -> usize {
    hits.get(&id).map_or(0, |v| v.iter().filter(|r| **r == row).count())
}

/// The hit effects of `effect` lie `radius` from a hit point of `row` (m_fHitEffectRandomOffsetRadius).
fn effect_offsets(r: &Run, row: &str, effect: &str) -> Vec<f32> {
    let hits: Vec<Vec3> = r.points.iter().filter(|(n, _)| *n == row).map(|p| p.1).collect();
    r.effects.iter().filter(|(n, _)| *n == effect).map(|(_, p)| hits.iter().map(|h| h.distance(*p)).fold(f32::MAX, f32::min)).collect()
}

#[test]
fn unison_thunder_hits_every_body_around_sora_once() {
    let r = run(24);
    assert!(r.history.contains(&0x41), "DoUThunder played: {:x?}", r.history);
    for (id, what) in [(FRONT, "in front"), (BESIDE, "beside him"), (BEHIND, "behind"), (DIAGONAL, "5 m away diagonally")] {
        assert_eq!(landed(&r.hits, id, "UNISON_THUNDER"), 1, "the column hits the body {what} once: {:?}", r.hits);
    }
    assert_eq!(landed(&r.hits, FAR, "UNISON_THUNDER"), 0, "15 m is outside its 8 m radius: {:?}", r.hits);
    // Its own hit effect at the impact (no random offset in TresProj_MagThunderBase).
    let d = effect_offsets(&r, "UNISON_THUNDER", "vs_thd_hit0");
    assert_eq!(d.len(), 4);
    assert!(d.iter().all(|d| *d < 0.01), "{d:?}");
}

#[test]
fn unison_fire_hits_every_body_around_sora_once_with_its_effect_40_cm_off() {
    let r = run(22);
    assert!(r.history.contains(&0x3F), "DoUFire played: {:x?}", r.history);
    for id in [FRONT, BESIDE, BEHIND, DIAGONAL] {
        assert_eq!(landed(&r.hits, id, "UNISON_FIRE"), 1, "dummy {id}: {:?}", r.hits);
    }
    assert_eq!(landed(&r.hits, FAR, "UNISON_FIRE"), 0, "{:?}", r.hits);
    // m_fHitEffectRandomOffsetRadius 40: each vs_fir_hit0 40 cm from its impact, in random directions.
    let d = effect_offsets(&r, "UNISON_FIRE", "vs_fir_hit0");
    assert_eq!(d.len(), 4);
    assert!(d.iter().all(|d| (d - 40.0).abs() < 0.05), "{d:?}");
}

#[test]
fn unison_blizzard_hits_around_sora_then_its_end_blast() {
    let r = run(23);
    assert!(r.history.contains(&0x40), "DoUBlizzard played: {:x?}", r.history);
    for id in [FRONT, BESIDE, BEHIND, DIAGONAL] {
        assert_eq!(landed(&r.hits, id, "UNISON_BLIZZARD"), 1, "dummy {id}: {:?}", r.hits);
        // UNISON_BLIZZARD_END: 800 cm from 0.4333 s, re-hit after 0.4 s, life 0.8 s: once.
        assert_eq!(landed(&r.hits, id, "UNISON_BLIZZARD_END"), 1, "dummy {id}: {:?}", r.hits);
    }
    assert_eq!(landed(&r.hits, FAR, "UNISON_BLIZZARD") + landed(&r.hits, FAR, "UNISON_BLIZZARD_END"), 0, "{:?}", r.hits);
    // The base's entry: vm_blz_hit0, 40 cm off the impact.
    let d = effect_offsets(&r, "UNISON_BLIZZARD", "vm_blz_hit0");
    assert!(!d.is_empty() && d.iter().all(|d| (d - 40.0).abs() < 0.05), "{d:?}");
}

#[test]
fn fusion_spin_is_carried_by_goofy() {
    // Goofy standing: the sphere (50 -> 400 at 0.25 s) at Sora's root + 150 reaches the three
    // bodies within 4.4 m, not the 5 m diagonal one.
    let r = run(25);
    assert!(r.history.contains(&0x42), "GooFSpin played: {:x?}", r.history);
    for id in [FRONT, BESIDE, BEHIND] {
        assert_eq!(landed(&r.hits, id, "FUSION_SPIN"), 1, "dummy {id}: {:?}", r.hits);
    }
    assert_eq!(landed(&r.hits, DIAGONAL, "FUSION_SPIN") + landed(&r.hits, FAR, "FUSION_SPIN"), 0, "{:?}", r.hits);
    assert!(r.effects.iter().any(|(n, _)| *n == "vn_ex002_frt_hit0"), "its m_HitEffects entry");
    // Goofy running along +X at 1000 cm/s: the attached actor goes with him (KeepWorldTransform):
    // the body 15 m ahead is reached before the 1.3 s life ends.
    let moving = run_with(25, 1000.0);
    assert_eq!(landed(&moving.hits, FAR, "FUSION_SPIN"), 1, "{:?}", moving.hits);
}

#[test]
fn fusion_rocket_hits_with_its_group_hit_effect() {
    let r = run(26);
    assert!(r.history.contains(&0x43), "GooFRocket played: {:x?}", r.history);
    assert!(r.hits.values().flatten().any(|row| *row == "FUSION_ROCKET"), "its SphereM window lands: {:?}", r.hits);
    assert!(r.effects.iter().any(|(n, _)| *n == "vn_ex002_frt_hit0"), "m_HitEffectGrpName FUSION_ROCKET");
}
