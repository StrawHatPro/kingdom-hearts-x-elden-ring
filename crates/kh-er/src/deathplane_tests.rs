//! (DEATHPLANE) The barrier rule for every kind of KH character, with the game's signal mocked (the kinds its contact
//! query would report), each through his own real death path where the crate has it: the leader Sora (kh-core), a friend
//! leader (kh-party through kh-partyhost's runtime and S2's leader body), an AI friend, AI Sora / Mimic Sora (a Sora core),
//! a character to come (a mock). And the game's fall damage stays discarded.

use super::*;
use crate::death::{leader_alive, Flow, Kind};
use crate::guards::{blame, Blame, BodyFall, FallGuard, NetVerdict};
use kh_core::{EventKind, Input, Pose, Sora, StateId, SORA_PARAMS};

const DT: f32 = 1.0 / 60.0;

/// What the game's query reports this frame (mocked): its kinds, as `game_side::touches` classifies them.
fn signal(kinds: &[i32]) -> Vec<Barrier> {
    kinds.iter().filter_map(|k| Barrier::of_kind(*k)).collect()
}

/// Sora with a real status (the level-99 loadout sheet, no ability that leaves him 1 HP), as death_tests.rs makes him.
fn real_sora(at: Vec3) -> Sora {
    real_sora_with(at, &[])
}

/// (KILLPLANE) The same Sora with these abilities equipped (sheet names: LAST_LEAVE, COMBO_LEAVE, LEAF_VEIL ...).
fn real_sora_with(at: Vec3, equipped: &[&str]) -> Sora {
    let path = format!("{}/../../sheets/port/sora-loadout-99.json", env!("CARGO_MANIFEST_DIR"));
    let json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))).unwrap();
    let mut loadout = kh_core::Loadout::new("sora-loadout-99");
    for row in json["rows"].as_array().unwrap() {
        let key = row[0].as_str().unwrap();
        use kh_core::status::LoadoutValue as V;
        let value = if key == "abilities.equipped" {
            V::List(equipped.iter().map(|s| s.to_string()).collect())
        } else {
            match &row[1] {
                serde_json::Value::Number(n) => V::Number(n.as_f64().unwrap()),
                serde_json::Value::String(t) => V::Text(t.clone()),
                serde_json::Value::Array(items) => V::List(items.iter().filter_map(|i| i.as_str().map(str::to_string)).collect()),
                serde_json::Value::Object(map) => V::Map(map.iter().filter_map(|(k, v)| v.as_f64().map(|n| (k.clone(), n))).collect()),
                _ => continue,
            }
        };
        loadout.set(key, &value).unwrap_or_else(|e| panic!("{e}"));
    }
    loadout.resolve().unwrap();
    let mut sora = Sora::new(SORA_PARAMS, Pose { position: at, yaw: 0.0 });
    *sora.status_mut() = kh_core::Status::from_loadout(&loadout);
    sora
}

// ------------------------------------------------------------------------------------------ the rule itself

#[test]
fn the_game_s_kinds_and_only_13_14_15_are_barriers() {
    assert_eq!(Barrier::of_kind(13), Some(Barrier::FallCamera));
    assert_eq!(Barrier::of_kind(14), Some(Barrier::LethalFall));
    assert_eq!(Barrier::of_kind(15), Some(Barrier::Kill));
    // the water's surface (21), Torrent's zones (27 / 28), the rest of the tracker's kinds and ordinary collision: none
    for k in [0, 8, 12, 16, 17, 20, 21, 23, 27, 28, 255] {
        assert_eq!(Barrier::of_kind(k), None, "{k}");
    }
}

#[test]
fn a_kill_plane_is_death_every_frame_until_he_is_down_then_nothing() {
    let mut d = Doom::default();
    assert!(d.touch(Barrier::Kill, false));
    assert!(!d.touch(Barrier::Kill, false), "the same doom");
    // given again every frame he still stands (a Second Chance, a dodge frame never saves him)
    for _ in 0..30 {
        assert_eq!(d.frame(true, false, DT), Some(Barrier::Kill));
    }
    assert!(d.void_is_death());
    // down: done, a clean slate
    assert_eq!(d.frame(false, true, DT), None);
    assert!(!d.void_is_death());
    assert_eq!(d.frame(true, true, DT), None);
    // a doom he outlives for HOLD s after the last touch (an invincibility longer than any): given up
    let mut d = Doom::default();
    d.touch(Barrier::Kill, true);
    let mut gave = 0;
    for _ in 0..((HOLD + 1.0) / DT) as usize {
        gave += u32::from(d.frame(true, true, DT).is_some());
    }
    assert!(gave as f32 * DT <= HOLD + DT && gave > 0 && !d.void_is_death(), "{gave}");
}

#[test]
fn a_lethal_fall_kills_at_his_landing_and_a_touch_without_a_fall_lapses() {
    // touched in the air: nothing while he falls, his death at the landing, then given as a kill until he is down
    let mut d = Doom::default();
    assert!(d.touch(Barrier::LethalFall, false));
    for _ in 0..120 {
        assert_eq!(d.frame(true, false, DT), None, "in the air");
        assert!(d.void_is_death(), "a pit under him: no put-back");
    }
    assert_eq!(d.frame(true, true, DT), Some(Barrier::LethalFall), "the landing");
    assert_eq!(d.frame(true, true, DT), Some(Barrier::Kill), "again until down");
    assert_eq!(d.frame(false, true, DT), None);
    // touched while standing at a rim: no fall follows -> it lapses after FLOOR_CLEAR
    let mut d = Doom::default();
    d.touch(Barrier::LethalFall, true);
    for _ in 0..((FLOOR_CLEAR + 0.1) / DT) as usize {
        assert_eq!(d.frame(true, true, DT), None);
    }
    assert!(!d.void_is_death());
    // touched while standing, then he steps off and lands below: death
    let mut d = Doom::default();
    d.touch(Barrier::LethalFall, true);
    assert_eq!(d.frame(true, true, DT), None);
    assert_eq!(d.frame(true, false, DT), None);
    assert_eq!(d.frame(true, false, DT), None);
    assert_eq!(d.frame(true, true, DT), Some(Barrier::LethalFall));
}

#[test]
fn the_fall_death_camera_alone_kills_nobody_but_the_void_after_it_is_death() {
    let mut d = Doom::default();
    // standing in the camera zone: nothing at all
    assert!(!d.touch(Barrier::FallCamera, true));
    assert!(!d.void_is_death() && d.frame(true, true, DT).is_none());
    // in the air through it: no death of its own, but no put-back from the void below
    assert!(d.touch(Barrier::FallCamera, false));
    for _ in 0..60 {
        assert_eq!(d.frame(true, false, DT), None);
    }
    assert!(d.void_is_death());
    assert_eq!(net(NetVerdict::PutBack, d.void_is_death()), NetVerdict::Death);
    // he lands (a ledge below): cleared, a later void is a put-back as before
    assert_eq!(d.frame(true, true, DT), None);
    assert!(!d.void_is_death());
    assert_eq!(net(NetVerdict::PutBack, d.void_is_death()), NetVerdict::PutBack);
    // the net's other verdicts are its own
    assert_eq!(net(NetVerdict::Nothing, true), NetVerdict::Nothing);
    assert_eq!(net(NetVerdict::Death, false), NetVerdict::Death);
}

#[test]
fn the_cast_covers_his_body_and_his_move_and_a_warp_is_not_swept() {
    // standing: from his head down to his feet (Sora 34 cm wide, 180 cm tall)
    let at = Vec3::new(10.0, 2.0, -5.0);
    let (o, d, r) = cast_of(at, at, 0.34, 1.8);
    assert!((r - 0.34).abs() < 1e-6);
    assert!((o - (at + Vec3::Y * 1.46)).length() < 1e-5 && (o + d - (at + Vec3::Y * 0.34)).length() < 1e-5);
    // falling 3 m in a frame: the sweep reaches his new feet
    let (o, d, _) = cast_of(at, at - Vec3::Y * 3.0, 0.34, 1.8);
    assert!(((o + d).y - (at.y - 3.0 + 0.34)).abs() < 1e-5);
    // nonsense sizes are clamped
    let (_, _, r) = cast_of(at, at, 9.0, f32::NAN);
    assert!((r - 0.6).abs() < 1e-6);
    // the sweep's start: last frame's place; after a warp, where he is
    let mut doom = Doom::default();
    assert_eq!(doom.sweep(at), (at, at));
    let next = at + Vec3::new(0.2, -0.5, 0.0);
    assert_eq!(doom.sweep(next), (at, next));
    let far = next + Vec3::X * 40.0;
    assert_eq!(doom.sweep(far), (far, far), "a warp");
    doom.pause();
    assert_eq!(doom.sweep(next), (next, next));
}

// ------------------------------------------------------------------------------------------ each kind of character

/// THE LEADER SORA, MOCKED SIGNAL "the sea's kill plane": the rule gives the net's / water's blow on his own core, his
/// KH3 knockdown plays, the death flow takes him down: KH3's game over (Continue = the last save, the HUD's).
#[test]
fn the_leader_sora_dies_in_the_sea_through_his_own_death_and_the_game_over() {
    let world = kh_core::test_world::TestWorld::floor();
    let mut sora = real_sora(Vec3::new(0.0, 0.0, 0.15));
    let mut plane = Deathplane::default();
    let mut flow: Flow<PartyChara> = Flow::default();
    let mut down_at = None;
    let mut died_event = false;
    for k in 0..240 {
        // under the surface from frame 60 on, the query reports kind 15 every frame (and the water's own 21)
        let kinds: &[i32] = if k >= 60 { &[21, 15] } else { &[] };
        let alive = sora.status().hp > 0;
        let (began, give) = plane.judge(Who::Leader, &signal(kinds), alive, sora.output().on_floor, DT);
        if k == 60 {
            assert_eq!(began, vec![Barrier::Kill]);
        }
        if give.is_some() {
            let feet = sora.output().position;
            kill_core(&mut sora, feet); // (KILLPLANE)
        }
        let out = sora.step(DT, &Input::default(), &world);
        let fell = out.events.iter().any(|e| matches!(e.kind, EventKind::PlayerDied { .. }));
        died_event |= fell;
        if flow.observe(PartyChara::Sora, leader_alive(None, sora.status().hp, fell), false).is_some() {
            down_at = Some(k);
        }
    }
    assert!(died_event && sora.status().hp <= 0, "KH3's own death: HP {}", sora.status().hp);
    assert_eq!(sora.output().state, StateId::Die, "his knockdown");
    assert!(down_at.is_some_and(|k| (60..64).contains(&k)), "down within a few frames of the touch: {down_at:?}");
    assert_eq!(flow.gameover(), Some(PartyChara::Sora), "KH3's game over for him");
    assert_eq!(flow.kind(), Some(Kind::Normal));
    // he is down: the doom is spent (no further blows)
    assert!(!plane.void_is_death(Who::Leader));
}

#[cfg(feature = "party")]
mod party_kinds {
    use super::*;
    use kh_partyhost::PartyRuntime;

    fn runtime(leader: Option<PartyChara>) -> PartyRuntime {
        let mut rt = PartyRuntime::new(kh_partyhost::RuntimeConfig { wanted: true, test: kh_partyhost::PartyTest::from_rows(&[]), weapons: Vec::new(), ai_pack: None, ai_pack_source: None });
        rt.spawn_one(PartyChara::Donald, Vec3::new(100.0, 0.0, 0.0), 0.0);
        rt.spawn_one(PartyChara::Goofy, Vec3::new(0.0, 100.0, 0.0), 0.0);
        rt.leader = leader.and_then(|c| rt.id(c));
        views(&mut rt);
        rt
    }

    fn views(rt: &mut PartyRuntime) {
        rt.current = rt.members.iter().filter_map(|(_, id)| rt.party.view(*id)).collect();
    }

    fn step(rt: &mut PartyRuntime, world: &kh_core::test_world::TestWorld) {
        let mut null = kh_core::NullHost::default();
        let mut host = kh_party::PartyHost { world, targets: &mut null.targets, audio: &mut null.audio, effects: &mut null.effects };
        rt.party.step(DT, &kh_party::SoraView::default(), &mut host);
        views(rt);
    }

    /// A FRIEND LEADER (Goofy, then Donald), MOCKED SIGNAL "the sea's kill plane": his KO through the party's Die state, the
    /// leader body's view says he is down, the death flow gives KH3's game over for him; Sora is untouched.
    #[test]
    fn a_friend_leader_dies_in_the_sea_his_ko_and_the_game_over() {
        for chara in [PartyChara::Goofy, PartyChara::Donald] {
            let world = kh_core::test_world::TestWorld::floor();
            let mut rt = runtime(Some(chara));
            let sora = real_sora(Vec3::new(0.0, -100.0, 0.15));
            let mut body = crate::party::leader::LeaderBody::default();
            body.update(&rt, &sora.output(), sora.status(), false);
            let mut plane = Deathplane::default();
            let mut flow: Flow<PartyChara> = Flow::default();
            let sora_hp = sora.status().hp;
            for k in 0..120 {
                let f = body.friend_lead().expect("a friend leads");
                assert_eq!(body.chara(), chara);
                let kinds: &[i32] = if k >= 30 { &[15] } else { &[] };
                let (_, give) = plane.judge(Who::Leader, &signal(kinds), f.alive, f.on_floor, DT);
                if give.is_some() {
                    let id = rt.leader.expect("his id");
                    knock_out(&mut rt.party, id);
                }
                step(&mut rt, &world);
                body.update(&rt, &sora.output(), sora.status(), false);
                let alive = leader_alive(body.friend_lead().map(|f| f.alive), sora.status().hp, false);
                flow.observe(body.chara(), alive, false);
            }
            let v = rt.view(chara).unwrap();
            assert!(v.ko && v.state == kh_party::NpcStateId::Die, "{}: KO'd", chara.name());
            assert_eq!(flow.gameover(), Some(chara), "{}: the game over is his", chara.name());
            assert_eq!(sora.status().hp, sora_hp, "Sora untouched");
        }
    }

    /// AN AI FRIEND PUSHED INTO THE SEA (Donald, Sora leading): his KO, through the party's own Die state; it is no death of
    /// the leader's (no game over); the other friend is untouched. And a guard does not save him: knock_out enters the KO.
    #[test]
    fn an_ai_friend_pushed_into_the_sea_is_knocked_out_and_it_is_no_game_over() {
        let world = kh_core::test_world::TestWorld::floor();
        let mut rt = runtime(None);
        let sora = real_sora(Vec3::new(0.0, -100.0, 0.15));
        let mut plane = Deathplane::default();
        let mut flow: Flow<PartyChara> = Flow::default();
        let mut kos = 0;
        for k in 0..120 {
            for (c, id) in rt.members.clone() {
                let v = rt.view(c).unwrap();
                let kinds: &[i32] = if c == PartyChara::Donald && k >= 20 { &[15] } else { &[] };
                let (_, give) = plane.judge(Who::Ally(c), &signal(kinds), !v.ko, v.on_ground, DT);
                if give.is_some() && knock_out(&mut rt.party, id) {
                    kos += 1;
                }
            }
            step(&mut rt, &world);
            flow.observe(PartyChara::Sora, leader_alive(None, sora.status().hp, false), false);
        }
        assert!(rt.view(PartyChara::Donald).unwrap().ko, "Donald KO");
        assert_eq!(kos, 1, "one KO: none given to a knocked-out friend");
        assert!(!rt.view(PartyChara::Goofy).unwrap().ko, "Goofy untouched");
        assert_eq!(flow.down(), None, "an AI friend's KO is no game over");
        // the KO itself, whatever would refuse a hit: Goofy in a state that refuses every hit (a ladder's leap) is not
        // hurt by a lethal hit through receive_hit, but a barrier's KO takes him all the same; a KO'd friend is not again
        let goofy = rt.id(PartyChara::Goofy).unwrap();
        rt.party.member_mut(goofy).unwrap().state = kh_party::NpcStateId::Ladder;
        let hp = rt.party.view(goofy).unwrap().hp;
        let lethal = kh_core::HitRecord { damage: hp as f32 + 1.0, attack_name: "test.lethal", ..kh_core::HitRecord::empty(kh_core::TargetId::NONE) };
        assert!(!rt.party.receive_hit(goofy, &lethal).accepted, "the hit path refuses him");
        assert_eq!(rt.party.view(goofy).unwrap().hp, hp);
        assert!(knock_out(&mut rt.party, goofy), "the barrier's KO does not");
        assert_eq!(rt.party.view(goofy).unwrap().state, kh_party::NpcStateId::Die);
        assert!(!knock_out(&mut rt.party, goofy), "already down");
    }

    /// AI SORA (a friend leads) AND MIMIC SORA: their own Sora core, the same blow: his knockdown (Mimic Sora's spirit.rs
    /// then sees him down: Died, the silver dissolve). A plain fall never touches them.
    #[test]
    fn ai_sora_and_mimic_sora_are_knocked_down_by_the_same_rule() {
        for who in [Who::Ally(PartyChara::Sora), Who::Mimic] {
            let world = kh_core::test_world::TestWorld::floor();
            let mut core = real_sora(Vec3::new(0.0, 0.0, 0.15));
            let mut plane = Deathplane::default();
            let mut died = false;
            for k in 0..120 {
                let kinds: &[i32] = if k >= 10 { &[14] } else { &[] };
                // he falls through a lethal-fall zone from frame 10 and lands at frame 40
                let on_floor = !(10..40).contains(&k);
                let (_, give) = plane.judge(who, &signal(kinds), core.status().hp > 0, on_floor, DT);
                if k < 40 {
                    assert!(give.is_none(), "{who:?}: nothing before the landing");
                }
                if give.is_some() {
                    let feet = core.output().position;
                    kill_core(&mut core, feet); // (KILLPLANE)
                }
                let out = core.step(DT, &Input::default(), &world);
                died |= out.events.iter().any(|e| matches!(e.kind, EventKind::PlayerDied { .. }));
            }
            assert!(died && core.status().hp <= 0 && core.output().state == StateId::Die, "{who:?}");
        }
    }
}

/// A CHARACTER TO COME (a mock): the rule knows nothing of him; his own death path is all he needs.
#[test]
fn a_character_to_come_gets_the_same_rule() {
    struct Riku {
        hp: i32,
    }
    let mut riku = Riku { hp: 300 };
    let mut plane = Deathplane::default();
    for k in 0..30 {
        let kinds: &[i32] = if k == 5 { &[15] } else { &[] };
        let (_, give) = plane.judge(Who::Leader, &signal(kinds), riku.hp > 0, true, DT); // (he leads)
        if give.is_some() {
            riku.hp = 0;
        }
    }
    assert_eq!(riku.hp, 0);
    plane.keep_only(&[]);
    assert!(!plane.void_is_death(Who::Leader));
}

// ------------------------------------------------------------------------------------------ the fall damage stays discarded

/// THE GAME'S FALL DAMAGE IS STILL NOT A KH CHARACTER'S (KH3 has none): a 70 m fall of the leader and the body's own
/// put-down are in the fall windows (what the game takes is discarded, `blame` = Fall), and with no barrier touched the
/// rule gives no death at the landing, whatever its height; the net puts him back from a plain void as before.
#[test]
fn an_ordinary_fall_is_still_discarded_and_is_never_a_barrier_death() {
    let mut guard = FallGuard::default();
    let mut body = BodyFall::default();
    let mut plane = Deathplane::default();
    guard.frame(true, 1956.0, DT);
    body.frame(true, Vec3::new(0.0, 19.56, 0.0), false, DT);
    let mut z = 1956.0;
    while z > -4748.0 {
        z -= 50.0;
        guard.frame(false, z, DT);
        body.frame(false, Vec3::new(0.0, z / 100.0, 0.0), false, DT);
        let (_, give) = plane.judge(Who::Leader, &signal(&[]), true, false, DT);
        assert!(give.is_none());
    }
    for _ in 0..5 {
        guard.frame(true, z, DT);
        body.frame(true, Vec3::new(0.0, z / 100.0, 0.0), false, DT);
        let (_, give) = plane.judge(Who::Leader, &signal(&[]), true, true, DT);
        assert!(give.is_none(), "no death at the landing");
    }
    assert!(guard.in_window(true), "the landing's fall damage: discarded");
    assert!(body.in_window());
    assert_eq!(blame(false, false, body.in_window(), false), Blame::Fall, "nobody blamed, discarded");
    assert!(!plane.void_is_death(Who::Leader));
    assert_eq!(net(NetVerdict::PutBack, plane.void_is_death(Who::Leader)), NetVerdict::PutBack, "a plain void: put back as before");
    // the same fall through a lethal-fall zone: the fall damage is still discarded (the guard does not change), the
    // barrier is the death
    let mut plane = Deathplane::default();
    plane.judge(Who::Leader, &signal(&[14]), true, false, DT);
    let (_, give) = plane.judge(Who::Leader, &signal(&[]), true, true, DT);
    assert_eq!(give, Some(Barrier::LethalFall));
    assert!(guard.in_window(true));
}

// ------------------------------------------------------------------------------------------ the game's code (offline)

/// THE ADDRESSES IN BOTH EXECUTABLES (the read-only copies the PORT check uses; a missing copy is skipped): the record and
/// kind functions and the tracker's constructor are where the table says, by their patterns alone too, and the
/// constructor's filter immediate is 0x66 in both.
#[cfg(windows)]
#[test]
fn the_barrier_functions_and_the_tracker_filter_in_both_executables() {
    use pelite::pe64::{Pe, PeFile};
    let mut checked = 0;
    for (rel, version) in [("build/sora_tmp/er/boss/eldenring.exe", crate::gamever::ExeVersion::V2710), ("build/sora_tmp/er/port272/eldenring_2720.exe", crate::gamever::ExeVersion::V2720)] {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(rel);
        let Ok(bytes) = std::fs::read(&path) else {
            println!("DEATHPLANE offline check: no {rel}: skipped");
            continue;
        };
        let pe = PeFile::from_bytes(&bytes).expect("a PE file");
        for item in &LOCATED {
            let at = crate::havok::resolve_in(pe, item, Some(version)).unwrap_or_else(|e| panic!("{e}"));
            assert_eq!(crate::havok::resolve_in(pe, item, None).ok(), Some(at), "{} by its pattern alone", item.name);
        }
        let ctor = crate::havok::resolve_in(pe, &TRACKER_CTOR, Some(version)).unwrap();
        let imm = pe.derva_slice::<u8>(ctor + FILTER_AT as u32, 4).unwrap();
        assert_eq!(u32::from_le_bytes([imm[0], imm[1], imm[2], imm[3]]), FILTER_RECORDED, "{}", version.name());
        checked += 1;
    }
    println!("DEATHPLANE offline check: {checked} executables checked");
}

/// (test aids) a one-shot fires on a change of its value, never at launch; a push is read absolute or relative.
#[test]
fn the_test_aids_fire_on_a_change_and_read_their_places() {
    let mut seen = None;
    assert_eq!(aid_fired(&mut seen, Some("40")), None, "the value at launch");
    assert_eq!(aid_fired(&mut seen, Some("40")), None);
    assert_eq!(aid_fired(&mut seen, Some("60")).as_deref(), Some("60"));
    assert_eq!(aid_fired(&mut seen, None), None, "taken away");
    assert_eq!(aid_fired(&mut seen, Some("60")).as_deref(), Some("60"), "written again");
    #[cfg(feature = "party")]
    {
        let feet = Vec3::new(10.0, 2.0, -3.0);
        assert_eq!(push_of("Donald, +5, 10, -2", feet), Some(("donald".to_string(), Vec3::new(15.0, 12.0, -5.0))));
        assert_eq!(push_of("goofy,1,2,3", feet), Some(("goofy".to_string(), Vec3::new(1.0, 2.0, 3.0))));
        assert_eq!(push_of("goofy,1,2", feet), None);
        assert_eq!(push_of(",1,2,3", feet), None);
    }
}

/// (the coordinator 2026-11-03) THE OLD DROWNING HEURISTIC IS RETIRED BY DEFAULT: with no key in the file `water.death` reads 0
/// (the key still brings it back), and the barrier rule is on (an absent `barrier.death` is on).
#[cfg(windows)]
#[test]
fn the_drowning_heuristic_is_off_by_default_and_the_barrier_rule_on() {
    let c = crate::control::Control::new(std::env::temp_dir().join("kh_deathplane_no_such_file.ini"));
    assert_eq!(c.flag("water.death"), Some(false));
    assert_ne!(c.flag("barrier.death"), Some(false));
}

// ------------------------------------------------------------------------------------------ (KILLPLANE) instant, past every ability

/// (KILLPLANE) What could keep a Sora core (the leader Sora, AI Sora, Mimic Sora) alive through a lethal hit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Saver {
    /// LAST_LEAVE: 1 HP from 2 or more.
    SecondChance,
    /// COMBO_LEAVE (KH3's "Once More"): 1 HP while its 0.6 s window runs, also from 1 HP.
    WithstandCombo,
    /// LEAF_VEIL: no damage during Cure.
    LeafBracer,
    /// The pawn's invincibility timer (the i-frames after shotlocks, Double Form ...).
    IFrames,
    /// The auto-revive item in the pouch (KH3's `Survive()`: half HP, 1 s of no damage).
    AutoRevive,
    /// The pawn's HP-limit hold (1 HP).
    HpLimitOne,
    /// The invincible pawn (a cinematic).
    Invincible,
}

const SAVERS: [Saver; 7] = [Saver::SecondChance, Saver::WithstandCombo, Saver::LeafBracer, Saver::IFrames, Saver::AutoRevive, Saver::HpLimitOne, Saver::Invincible];

/// A Sora core on the floor with `saver` in effect at the moment of the next step's hit.
fn saved_sora(saver: Saver, world: &kh_core::test_world::TestWorld) -> Sora {
    let equipped: &[&str] = match saver {
        Saver::SecondChance => &["LAST_LEAVE"],
        Saver::WithstandCombo => &["COMBO_LEAVE"],
        Saver::LeafBracer => &["LEAF_VEIL"],
        _ => &[],
    };
    let mut s = real_sora_with(Vec3::new(0.0, 0.0, Sora::origin_height_on_floor(&SORA_PARAMS)), equipped);
    for _ in 0..5 {
        s.step(DT, &Input::default(), world);
    }
    match saver {
        Saver::SecondChance => {}
        Saver::WithstandCombo => {
            // from 1 HP only Withstand Combo saves him; its window armed as by a hit just before
            s.status_mut().set_hp(1);
            s.status_mut().combo_leave_timer = 0.6;
        }
        Saver::LeafBracer => {
            let mp = s.status().max_mp();
            s.status_mut().set_mp(mp);
            s.command_request(kh_core::command::CommandRequest::Execute(kh_core::magic::spells::cmd::CURE));
            let mut cast = false;
            for _ in 0..10 {
                cast |= s.step(DT, &Input::default(), world).state == StateId::Firemagic;
                if cast {
                    break;
                }
            }
            assert!(cast, "Cure is cast");
            for _ in 0..6 {
                s.step(DT, &Input::default(), world);
            }
        }
        Saver::IFrames => s.combat_mut().invincible_timer = 2.0,
        Saver::AutoRevive => s.status_mut().items.push(kh_core::status::ItemStack { item: "ITEM_KUPOCOIN".to_string(), count: 1 }),
        Saver::HpLimitOne => s.status_mut().hp_limit_one = true,
        Saver::Invincible => s.status_mut().invincible = true,
    }
    s
}

fn lethal_hit(s: &Sora) -> kh_core::states::HitOnPlayer {
    kh_core::states::HitOnPlayer::new(kh_core::enums::ETresDamageKind::Small, 1.0e6, kh_core::TargetId::NONE, s.output().position + Vec3::X * 100.0)
}

/// (KILLPLANE, THE USER 2026-11-06) EVERY SAVER, BOTH WAYS: a normal lethal hit still respects it (he lives), a kill plane's
/// death (`kill_core`) does not: HP 0 and `PlayerDied` in the very next step, the Die state, the auto-revive item untouched.
#[test]
fn a_kill_plane_kills_a_sora_core_instantly_whatever_saves_him_from_a_hit() {
    let world = kh_core::test_world::TestWorld::floor();
    for saver in SAVERS {
        // a normal lethal hit: the saver keeps him alive
        let mut s = saved_sora(saver, &world);
        let hit = lethal_hit(&s);
        s.take_hit(hit);
        let out = s.step(DT, &Input::default(), &world);
        assert!(s.status().hp > 0 && !out.events.iter().any(|e| matches!(e.kind, EventKind::PlayerDied { .. })), "{saver:?}: a normal hit is survived (HP {})", s.status().hp);
        // the kill plane: dead at once
        let mut s = saved_sora(saver, &world);
        let coins = s.status().items.iter().map(|i| i.count).sum::<u32>();
        let feet = s.output().position;
        kill_core(&mut s, feet);
        let out = s.step(DT, &Input::default(), &world);
        assert_eq!(s.status().hp, 0, "{saver:?}: HP 0 in the very next step");
        assert!(out.events.iter().any(|e| matches!(e.kind, EventKind::PlayerDied { .. })), "{saver:?}: his death event in that step");
        let out = s.step(DT, &Input::default(), &world);
        assert_eq!(out.state, StateId::Die, "{saver:?}: KH3's knockdown");
        assert_eq!(s.status().items.iter().map(|i| i.count).sum::<u32>(), coins, "{saver:?}: no auto-revive item taken");
        // a lethal hit and a second kill on top change nothing
        let hit = lethal_hit(&s);
        s.take_hit(hit);
        kill_core(&mut s, feet);
        for _ in 0..30 {
            s.step(DT, &Input::default(), &world);
        }
        assert_eq!((s.status().hp, s.output().state), (0, StateId::Die), "{saver:?}");
    }
}

/// (KILLPLANE) A Sora core killed by a kill plane STAYS DOWN until his host stands him up: a friend's Cure, an item or any
/// Recover given to him meanwhile does not lift his HP (AI Sora while a friend leads; the leader under the game over).
#[test]
fn a_sora_core_killed_by_a_kill_plane_is_not_healed_back() {
    let world = kh_core::test_world::TestWorld::floor();
    let mut s = saved_sora(Saver::SecondChance, &world);
    let feet = s.output().position;
    kill_core(&mut s, feet);
    for k in 0..300 {
        if k % 50 == 10 {
            s.status_request(kh_core::StatusRequest::Recover { hp: 500, mp: 0, focus: 0, kind: 1 });
        }
        s.step(DT, &Input::default(), &world);
        assert_eq!(s.status().hp, 0, "step {k}: no heal lifts a kill plane's death");
    }
    assert_eq!(s.output().state, StateId::Die);
}

/// (KILLPLANE) THE LEADER SORA WITH EVERY SAVER, IN THE SEA'S KILL PLANE: down the frame of the touch, KH3's game over (the
/// party's death, a NORMAL one even at a scripted death's place: `leader_killed`), never the 1-HP spare.
#[test]
fn the_leader_sora_with_any_saver_dies_at_the_touch_and_it_is_the_party_s_death() {
    let world = kh_core::test_world::TestWorld::floor();
    for saver in SAVERS {
        let mut sora = saved_sora(saver, &world);
        let mut plane = Deathplane::default();
        let mut flow: Flow<PartyChara> = Flow::default();
        let mut down_at = None;
        for k in 0..60 {
            let kinds: &[i32] = if k >= 20 { &[21, 15] } else { &[] };
            let (_, give) = plane.judge(Who::Leader, &signal(kinds), sora.status().hp > 0, sora.output().on_floor, DT);
            if give.is_some() {
                plane.leader_killed = true;
                let feet = sora.output().position;
                kill_core(&mut sora, feet);
            }
            let out = sora.step(DT, &Input::default(), &world);
            let fell = out.events.iter().any(|e| matches!(e.kind, EventKind::PlayerDied { .. }));
            // as at the tutorial boss's place: a kill plane's death there is still a normal one
            if flow.observe(PartyChara::Sora, leader_alive(None, sora.status().hp, fell), !plane.leader_killed).is_some() {
                down_at = Some(k);
            }
        }
        assert_eq!(down_at, Some(20), "{saver:?}: down in the frame of the touch");
        assert_eq!((flow.kind(), flow.gameover()), (Some(Kind::Normal), Some(PartyChara::Sora)), "{saver:?}: the party's death, KH3's game over");
    }
}

#[cfg(feature = "party")]
mod killplane_party {
    use super::*;
    use kh_core::enums::ETresAbilityKind as Ab;
    use kh_partyhost::PartyRuntime;

    fn runtime(leader: Option<PartyChara>) -> PartyRuntime {
        let mut rt = PartyRuntime::new(kh_partyhost::RuntimeConfig { wanted: true, test: kh_partyhost::PartyTest::from_rows(&[]), weapons: Vec::new(), ai_pack: None, ai_pack_source: None });
        rt.spawn_one(PartyChara::Donald, Vec3::new(100.0, 0.0, 0.0), 0.0);
        rt.spawn_one(PartyChara::Goofy, Vec3::new(0.0, 100.0, 0.0), 0.0);
        rt.leader = leader.and_then(|c| rt.id(c));
        views(&mut rt);
        rt
    }

    fn views(rt: &mut PartyRuntime) {
        rt.current = rt.members.iter().filter_map(|(_, id)| rt.party.view(*id)).collect();
    }

    fn step(rt: &mut PartyRuntime, world: &kh_core::test_world::TestWorld) {
        let mut null = kh_core::NullHost::default();
        let mut host = kh_party::PartyHost { world, targets: &mut null.targets, audio: &mut null.audio, effects: &mut null.effects };
        rt.party.step(DT, &kh_party::SoraView::default(), &mut host);
        views(rt);
    }

    /// Donald's and Goofy's own Second Chance and Withstand Combo (Donald: his overhaul abilities; Goofy: KH3's).
    fn give_survival(rt: &mut PartyRuntime, c: PartyChara) {
        let id = rt.id(c).unwrap();
        let s = &mut rt.party.member_mut(id).unwrap().status;
        match c {
            PartyChara::Donald => {
                let d = s.donald.as_mut().expect("Donald's overhaul state");
                for k in [kh_party::donald::DonAbility::SecondChance, kh_party::donald::DonAbility::WithstandCombo] {
                    d.abilities.give(k);
                    d.abilities.set_equipped(k, true);
                }
            }
            _ => {
                s.abilities.set(Ab::LastLeave, 1, 1);
                s.abilities.set(Ab::ComboLeave, 1, 1);
            }
        }
        assert!(s.count(Ab::LastLeave) > 0 && s.count(Ab::ComboLeave) > 0, "{}: Second Chance and Withstand Combo", c.name());
    }

    /// (KILLPLANE) DONALD AND GOOFY WITH THEIR OWN SECOND CHANCE AND WITHSTAND COMBO: a normal lethal hit leaves them 1 HP; a
    /// kill plane knocks them out AT ONCE (in the call: HP 0, the Die state), and they STAY DOWN past their revive timer, a
    /// heal or an item given meanwhile, until the next host's `revive_all` (the respawn) stands them up.
    #[test]
    fn friends_with_their_survival_abilities_are_knocked_out_at_once_and_stay_down_until_the_respawn() {
        let world = kh_core::test_world::TestWorld::floor();
        for c in [PartyChara::Donald, PartyChara::Goofy] {
            // a normal lethal hit: Second Chance holds
            let mut rt = runtime(None);
            give_survival(&mut rt, c);
            step(&mut rt, &world);
            let id = rt.id(c).unwrap();
            let hp = rt.view(c).unwrap().hp;
            assert!(hp > 1);
            let lethal = kh_core::HitRecord { damage: hp as f32 * 10.0, attack_name: "test.lethal", ..kh_core::HitRecord::empty(kh_core::TargetId::NONE) };
            assert!(rt.party.receive_hit(id, &lethal).accepted, "{}: the hit lands", c.name());
            for _ in 0..30 {
                step(&mut rt, &world);
            }
            let v = rt.view(c).unwrap();
            assert!(v.hp >= 1 && !v.ko, "{}: Second Chance kept him up from a normal hit (HP {}, KO {})", c.name(), v.hp, v.ko);
            // the kill plane: down in the call, whatever he has
            let mut rt = runtime(None);
            give_survival(&mut rt, c);
            step(&mut rt, &world);
            let id = rt.id(c).unwrap();
            assert!(knock_out(&mut rt.party, id), "{}: down now", c.name());
            let m = rt.party.member(id).unwrap();
            assert!(m.status.hp == 0 && m.state == kh_party::NpcStateId::Die && m.ko_held(), "{}: HP 0, the KO, held", c.name());
            // past his revive timer (Donald 40 s, Goofy 35 s), with a heal every 10 s: still down
            let revive = m.status.revive_time();
            let steps = ((revive + 10.0) / DT) as usize;
            for k in 0..steps {
                if k % 600 == 300 {
                    rt.party.member_mut(id).unwrap().status.hp = 80;
                }
                step(&mut rt, &world);
            }
            let v = rt.view(c).unwrap();
            assert!(v.ko && v.hp == 0, "{}: still down {:.0} s later (KO {}, HP {})", c.name(), revive + 10.0, v.ko, v.hp);
            // the respawn: everyone stands again, full
            assert_eq!(crate::death::revive_all(&mut rt), vec![c], "{}: the respawn stands him up", c.name());
            let m = rt.party.member(id).unwrap();
            assert!(m.state != kh_party::NpcStateId::Die && m.status.hp == m.status.max_hp() && !m.ko_held());
        }
    }

    /// (KILLPLANE) A friend in a lethal blow's dying reaction (thrown, his KO to come at the landing) is knocked out at the
    /// touch all the same, and held; an ordinary KO (no kill plane) still revives by its timer.
    #[test]
    fn a_dying_friend_is_taken_at_the_touch_and_an_ordinary_ko_still_revives() {
        let world = kh_core::test_world::TestWorld::floor();
        let mut rt = runtime(None);
        step(&mut rt, &world);
        let goofy = rt.id(PartyChara::Goofy).unwrap();
        {
            let m = rt.party.member_mut(goofy).unwrap();
            m.status.hp = 0;
            m.hurt.dying = true;
        }
        assert!(knock_out(&mut rt.party, goofy), "the dying reaction is no shelter");
        assert!(rt.party.member(goofy).unwrap().ko_held());
        // an ordinary KO: Donald down by the party's own Die state revives when his timer runs out
        let donald = rt.id(PartyChara::Donald).unwrap();
        let revive = {
            let m = rt.party.member_mut(donald).unwrap();
            m.status.hp = 0;
            m.start_die();
            m.status.revive_time()
        };
        for _ in 0..(((revive + 8.0) / DT) as usize) {
            step(&mut rt, &world);
        }
        assert!(!rt.view(PartyChara::Donald).unwrap().ko, "an ordinary KO revives by its timer");
        assert!(rt.view(PartyChara::Goofy).unwrap().ko, "the kill plane's KO does not");
    }

    /// (KILLPLANE) A FRIEND LEADER (Goofy, then Donald) WITH SECOND CHANCE AND WITHSTAND COMBO IN THE SEA'S KILL PLANE: his KO
    /// in the frame of the touch and the party's death (KH3's game over for him, a NORMAL one), no swap to the next leader.
    #[test]
    fn a_friend_leader_with_survival_abilities_dies_at_the_touch_and_it_is_the_party_s_death() {
        for chara in [PartyChara::Goofy, PartyChara::Donald] {
            let world = kh_core::test_world::TestWorld::floor();
            let mut rt = runtime(Some(chara));
            give_survival(&mut rt, chara);
            let sora = real_sora(Vec3::new(0.0, -100.0, 0.15));
            let mut body = crate::party::leader::LeaderBody::default();
            body.update(&rt, &sora.output(), sora.status(), false);
            let mut plane = Deathplane::default();
            let mut flow: Flow<PartyChara> = Flow::default();
            let mut down_at = None;
            for k in 0..60 {
                let f = body.friend_lead().expect("a friend leads");
                let kinds: &[i32] = if k >= 30 { &[15] } else { &[] };
                let (_, give) = plane.judge(Who::Leader, &signal(kinds), f.alive, f.on_floor, DT);
                if give.is_some() {
                    plane.leader_killed = true;
                    let id = rt.leader.expect("his id");
                    knock_out(&mut rt.party, id);
                }
                step(&mut rt, &world);
                body.update(&rt, &sora.output(), sora.status(), false);
                let alive = leader_alive(body.friend_lead().map(|f| f.alive), sora.status().hp, false);
                if flow.observe(body.chara(), alive, !plane.leader_killed).is_some() {
                    down_at = Some(k);
                }
            }
            assert_eq!(down_at, Some(30), "{}: down in the frame of the touch", chara.name());
            assert_eq!(body.chara(), chara, "{}: no swap to the next leader", chara.name());
            assert_eq!((flow.kind(), flow.gameover()), (Some(Kind::Normal), Some(chara)), "{}: the party's death", chara.name());
        }
    }
}
