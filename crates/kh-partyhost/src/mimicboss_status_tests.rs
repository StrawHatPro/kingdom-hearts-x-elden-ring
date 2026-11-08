//! (MIMIC-BOSS hotfix 3, MB3-FREEZE; THE USER 2026-11-06: "While playing as Sora, I also froze in place for awhile after
//! getting hit by the mimic tear's blizzard spell") A copy's statuses on the party member it fights, through the real
//! cores: the hit as the copy's targets keep it ([`BossTargets`]), then the member's own damage path as the host gives it
//! (Sora's `take_hit(player_hit_of)`, a friend's `receive_hit(party_hit_of)`). The member takes KH3's PLAYER-side status
//! ([`PLAYER_STATUS_LONGEST`]), never the time the row gives an enemy.

use kh_core::combat::badstat::kind;
use kh_core::enums::ETresDamageKind;
use kh_core::host::{HitInfo, HitRecord};
use kh_core::magic::spells::attack_row;
use kh_core::test_world::TestWorld;
use kh_core::{Input, Pose, Sora, StateId, Vec3, SORA_PARAMS};
use kh_host::formation::Formation;
use kh_host::party::PartyChara;

use crate::mimicboss::{party_hit_of, player_hit_of, player_side_status, victim_id, victim_of_friend, victim_of_sora, BossTargets, Victim, PLAYER_STATUS_LONGEST};
use crate::*;

const DT: f32 = 1.0 / 60.0;
const COPY_AT: Vec3 = Vec3::new(200.0, 0.0, 0.0);

/// The copy's hit on `chara` as kh-core's hit path makes it from a row: its damage, kind and status as the row has them.
fn copy_hit(chara: PartyChara, kind: ETresDamageKind, status: &'static str, time: f32) -> HitRecord {
    let mut h = HitRecord::empty(victim_id(chara));
    h.damage = 4.0;
    h.damage_kind = kind;
    h.direction = Vec3::new(-1.0, 0.0, 0.0);
    h.position = Vec3::new(0.0, 0.0, 90.0);
    h.info = HitInfo { bad_status: status, bad_time: time, ..HitInfo::DEFAULT };
    h
}

/// The hit as the copy's targets keep it for the host (the boss's attack rate 1).
fn through_targets(victim: Victim, hit: &HitRecord) -> HitRecord {
    let mut t = BossTargets { victims: vec![victim], hits: Vec::new(), attack_rate: 1.0, orders: Vec::new() };
    assert!(kh_core::Targets::apply_hit(&mut t, hit).accepted, "the copy's hit lands on its victim");
    assert_eq!(t.hits.len(), 1);
    t.hits[0]
}

/// Sora at level 99 (sheets/port/sora-loadout-99.json: no FREEZE_GUARD / STUN_GUARD, abilities 66 / 68).
fn level99() -> kh_core::Status {
    use kh_core::status::LoadoutValue;
    let path = format!("{}/../../sheets/port/sora-loadout-99.json", env!("CARGO_MANIFEST_DIR"));
    let json: serde_json::Value = serde_json::from_str(std::fs::read_to_string(&path).unwrap().trim_start_matches('\u{feff}')).unwrap();
    let mut loadout = kh_core::Loadout::new("sora-loadout-99");
    for row in json["rows"].as_array().unwrap() {
        let v = match &row[1] {
            serde_json::Value::Number(n) => LoadoutValue::Number(n.as_f64().unwrap()),
            serde_json::Value::String(s) => LoadoutValue::Text(s.clone()),
            serde_json::Value::Array(a) => LoadoutValue::List(a.iter().map(|i| i.as_str().unwrap().to_string()).collect()),
            serde_json::Value::Object(m) => LoadoutValue::Map(m.iter().map(|(k, v)| (k.clone(), v.as_f64().unwrap())).collect()),
            _ => continue,
        };
        loadout.set(row[0].as_str().unwrap(), &v).unwrap();
    }
    loadout.resolve().unwrap();
    kh_core::Status::from_loadout(&loadout)
}

/// The leader Sora standing on the floor.
fn standing_sora(world: &TestWorld) -> Sora {
    let mut s = Sora::new(SORA_PARAMS, Pose { position: Vec3::ZERO, yaw: 0.0 }).with_status(level99());
    assert!(!s.status().resists_bad_status(kind::FREEZE) && !s.status().resists_bad_status(kind::STUN), "no guard ability");
    for _ in 0..20 {
        s.step(DT, &Input::default(), world);
    }
    s
}

#[test]
fn status_the_player_side_rule_bounds_each_kind_by_kh3s_longest_on_the_player() {
    let cases: [(&'static str, f32, &'static str, f32); 10] = [
        ("FREEZE", 5.0, "FREEZE", 2.0),       // Sora's Blizzard (p_ex001 MAG_BLIZZARD)
        ("FREEZE", 10.0, "FREEZE", 2.0),      // Blizzaga / Blizzagan, Donald's Frozen of his status book
        ("FREEZE", 1.0, "FREEZE", 1.0),       // a shorter time stays as it is
        ("STUN", 30.0, "STUN", 8.0),          // a Link row's stun
        ("BURN", 7.0, "BURN", 5.0),
        ("DISCHARGE", 7.0, "DISCHARGE", 7.0), // Sora's Thundara: inside KH3's player range
        ("TIME_BREAK", 12.0, "TIME_BREAK", 10.0),
        ("HONEY", 8.0, "NONE", 0.0),          // only Sora's own form rows carry it: KH3 never puts it on the player
        ("STOP", 0.3, "NONE", 0.0),
        ("NONE", 0.0, "NONE", 0.0),
    ];
    for (status, time, want, want_time) in cases {
        let mut h = copy_hit(PartyChara::Sora, ETresDamageKind::Small, status, time);
        let changed = player_side_status(&mut h);
        assert_eq!((h.info.bad_status, h.info.bad_time), (want, want_time), "{status} {time} s");
        assert_eq!(changed.is_some(), want_time != time, "{status} {time} s: reported");
    }
    // the hits that keep no status as a pawn status pass untouched
    let mut catch = copy_hit(PartyChara::Sora, ETresDamageKind::Small, "CATCH", 4.0);
    assert!(player_side_status(&mut catch).is_none());
    assert_eq!((catch.info.bad_status, catch.info.bad_time), ("CATCH", 4.0));
}

/// The table is KH3's own data: the longest time of each kind on the damaging rows of the player-facing attack tables.
#[test]
fn status_the_longest_player_side_times_are_kh3s_own_data() {
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../sheets/entities/DataTable");
    if !base.is_dir() {
        eprintln!("no sheets/entities/DataTable: skipped");
        return;
    }
    let mut longest: std::collections::BTreeMap<u8, f32> = Default::default();
    // (MIMIC-BOSS hotfix 3, the coordinator's decision) STUN: the enemies' rows alone; FREEZE: a Keyblade enemy's Blizzard
    let mut stun_enemies = 0.0f32;
    let mut kb_blizzard = None;
    let mut dirs: Vec<std::path::PathBuf> = ["Enemy", "Enemy_Ship", "Gimmick", "Vehicle"].iter().map(|d| base.join(d)).collect();
    let mut files = 0;
    while let Some(d) = dirs.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                dirs.push(p);
                continue;
            }
            if p.extension().and_then(|x| x.to_str()) != Some("json") {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&p) else { continue };
            let Ok(j) = serde_json::from_str::<serde_json::Value>(text.trim_start_matches('\u{feff}')) else { continue };
            let Some(rows) = j["rows"].as_array() else { continue };
            files += 1;
            let gimmick = p.components().any(|c| c.as_os_str() == "Gimmick");
            let e_ex360 = p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with("e_ex360_AttackDataTable"));
            for r in rows {
                let name = r["m_BadStatusKind"].as_str().unwrap_or("").rsplit("TRES_BADSTAT_").next().unwrap_or("");
                let k = kh_core::combat::badstat::kind_of(name);
                let (power, rate, time) = (r["m_Power"].as_f64().unwrap_or(0.0), r["m_BadStatusGenRate"].as_f64().unwrap_or(0.0), r["m_BadStatusEffectTime"].as_f64().unwrap_or(0.0));
                if k == 0 || power <= 0.0 || rate <= 0.0 || time <= 0.0 {
                    continue;
                }
                let e = longest.entry(k).or_insert(0.0);
                *e = e.max(time as f32);
                if k == kind::STUN && !gimmick {
                    stun_enemies = stun_enemies.max(time as f32);
                }
                if e_ex360 && r["_row_name"].as_str() == Some("Attack4_Blizzard") {
                    kb_blizzard = Some(time as f32);
                }
            }
        }
    }
    assert!(files > 100, "the attack tables are there ({files} files)");
    assert_eq!(longest.get(&kind::FREEZE), Some(&3.0), "the longest freeze on the player (e_ex301)");
    assert_eq!(longest.get(&kind::STUN), Some(&10.0), "the longest stun with the gimmicks (g_bx_blowingcar)");
    longest.insert(kind::FREEZE, kb_blizzard.expect("e_ex360 Attack4_Blizzard"));
    longest.insert(kind::STUN, stun_enemies);
    let table: std::collections::BTreeMap<u8, f32> = PLAYER_STATUS_LONGEST.iter().copied().collect();
    assert_eq!(longest, table, "KH3's data vs the table");
}

#[test]
fn status_a_copys_blizzard_freezes_the_leader_sora_for_kh3s_player_time_not_the_enemy_rows() {
    let world = TestWorld::floor();
    for row in ["MAG_BLIZZARD", "MAG_BLIZZARA", "MAG_BLIZZAGA"] {
        let r = attack_row(row).expect("the row");
        assert_eq!(r.bad_status, "FREEZE");
        assert!(r.bad_time >= 5.0, "{row}: the row's time on an enemy is {} s", r.bad_time);
        let mut s = standing_sora(&world);
        let hit = through_targets(victim_of_sora(&s, COPY_AT), &copy_hit(PartyChara::Sora, r.kind, r.bad_status, r.bad_time));
        let on_him = player_hit_of(&hit, COPY_AT, 180.0);
        assert_eq!((on_him.bad_status, on_him.bad_time), ("FREEZE", 2.0), "{row}: a Keyblade enemy's Blizzard on the player (e_ex360)");
        s.take_hit(on_him);
        let mut frozen = 0;
        for _ in 0..1200 {
            // he mashes A as the user did: KH3's freeze takes no input
            let a = if frozen % 8 == 0 { kh_core::PadButtons::NONE.with(kh_core::PadButton::FaceBottom) } else { kh_core::PadButtons::NONE };
            let out = s.step(DT, &Input::default().with_pad(kh_core::PadState { held: a, pressed: a, released: kh_core::PadButtons::NONE }), &world);
            if out.state == StateId::BadstatFreeze {
                frozen += 1;
            } else if frozen > 0 {
                break;
            }
        }
        assert!((115..=122).contains(&frozen), "{row}: frozen {frozen} steps (2 s, never the row's {} s)", r.bad_time);
        assert_eq!(s.combat().bad.kind, 0, "{row}: the ice broke and nothing is left on him");
    }
}

#[test]
fn status_a_copys_long_stun_holds_the_leader_sora_at_most_kh3s_longest() {
    let world = TestWorld::floor();
    let mut s = standing_sora(&world);
    let hit = through_targets(victim_of_sora(&s, COPY_AT), &copy_hit(PartyChara::Sora, ETresDamageKind::Small, "STUN", 30.0));
    let on_him = player_hit_of(&hit, COPY_AT, 180.0);
    assert_eq!(on_him.bad_time, 8.0);
    s.take_hit(on_him);
    let mut stunned = 0;
    for _ in 0..2400 {
        let out = s.step(DT, &Input::default(), &world);
        if out.state == StateId::BadstatStun {
            stunned += 1;
        } else if stunned > 0 {
            break;
        }
    }
    assert!(stunned > 0, "KH3's player stun state (122)");
    // the status counted inside the state (8 s), then its end clip (1.07 s)
    assert!(stunned <= 8 * 60 + 70, "stunned {stunned} steps (8 s and the end clip, never 30 s)");
}

#[test]
fn status_a_copys_honey_never_reaches_the_leader_sora() {
    let world = TestWorld::floor();
    let mut s = standing_sora(&world);
    // p_ex001 BZ2_SHOT (Hunny Spout): HONEY 100 %, 8 s
    let hit = through_targets(victim_of_sora(&s, COPY_AT), &copy_hit(PartyChara::Sora, ETresDamageKind::Small, "HONEY", 8.0));
    s.take_hit(player_hit_of(&hit, COPY_AT, 180.0));
    for _ in 0..30 {
        s.step(DT, &Input::default(), &world);
    }
    assert_eq!((s.combat().bad.kind, s.combat().bad.dilation), (0, 1.0), "no honey on him");
}

#[test]
fn status_a_leading_donalds_freeze_from_his_copy_is_kh3s_player_time() {
    let mut rt = PartyRuntime::new(RuntimeConfig { wanted: true, test: PartyTest::from_rows(&[]), weapons: Vec::new(), ai_pack: None, ai_pack_source: None });
    let mut lead_sora = Sora::new(SORA_PARAMS, Pose { position: Vec3::ZERO, yaw: 0.0 });
    let world = TestWorld::floor();
    let step = |rt: &mut PartyRuntime, s: &mut Sora, asked: Option<Formation>| {
        let cam = kh_camera::CameraView { position: Vec3::new(-600.0, 0.0, 200.0), rotation: Default::default(), rotation_target: Default::default(), look_point: Vec3::ZERO, distance: 600.0, auto_power: 0.0, stick_wait: 0.0, still_wait: 0.0, reset_wait: 0.0, slope_pitch: 0.0, stick_active: false, followed: false, collided: false };
        let mut targets = BossTargets::default();
        let input = Input::default();
        let _ = rt.route(RouteIn { sora: s, input: &input, dt: DT, targets: &targets, camera: &cam, asked_formation: asked, asked_ai: Vec::new(), nav: None, now: 0.0 });
        let out = s.output();
        let (mut audio, mut effects) = (kh_core::host::audio::NullAudio, kh_core::host::effects::NullEffects);
        rt.step(StepIn { dt: DT, sora: s, out: &out, lock_on: kh_core::TargetId::NONE, camera: &cam, world: &world, targets: &mut targets, audio: &mut audio, effects: &mut effects, nav: None, now: 0.0 });
        let _ = rt.after_step(&targets, DT, 0.0);
    };
    step(&mut rt, &mut lead_sora, None);
    step(&mut rt, &mut lead_sora, Some(Formation::new(&[PartyChara::Sora, PartyChara::Donald, PartyChara::Goofy], PartyChara::Donald).unwrap()));
    for _ in 0..3 {
        step(&mut rt, &mut lead_sora, None);
    }
    assert_eq!(rt.leader_chara(), PartyChara::Donald);
    let victim = victim_of_friend(&rt, PartyChara::Donald, COPY_AT).expect("Donald in the party");
    // the Donald copy's Freeze: the Frozen time of its status book (an enemy's), 10 s here
    let hit = through_targets(victim, &copy_hit(PartyChara::Donald, ETresDamageKind::Small, "FREEZE", 10.0));
    let _ = rt.receive_hit(PartyChara::Donald, &party_hit_of(&hit));
    let id = rt.id(PartyChara::Donald).unwrap();
    assert_eq!(rt.party.member(id).unwrap().bad, (kind::FREEZE, 2.0), "his pawn holds the Keyblade enemy's Blizzard time");
    let mut frozen = 0;
    for _ in 0..1200 {
        step(&mut rt, &mut lead_sora, None);
        let st = rt.party.member(id).unwrap().state;
        if st == kh_party::NpcStateId::BadStatFreeze {
            frozen += 1;
        } else if frozen > 0 {
            break;
        }
    }
    assert!(frozen > 0, "TresNpcState_BadStatFreeze");
    assert!(frozen <= 3 * 60 + 2, "Donald frozen {frozen} steps (at most 3 s, never 10 s)");
}
