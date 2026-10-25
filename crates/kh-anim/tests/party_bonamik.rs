//! (party A2) The friends' Bonamik notifies: ResetAndPreRoll's reset + pre-roll (0x140548a20), the notify states of the
//! swim / reaction clips (EnableGroundCollision, SpeedRate, ChangeGroupParams, CharaWind), and the ground clamp.
//!
//! `cargo test --release -p kh-anim --test party_bonamik -- --nocapture`

use std::path::PathBuf;

use glam::Vec3;
use kh_anim::{Animator, BodySlot, Frame, NoIkWorld, Pack, Settings, Trs};

fn animator(npc: &str) -> Option<Animator> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_pack/party").join(npc);
    if !dir.join("manifest.json").is_file() {
        println!("no {} (node tools/party_pack.js): skipped", dir.display());
        return None;
    }
    Some(Animator::new(Pack::load(&dir).unwrap(), Settings { leg_ik: false, ..Settings::default() }).unwrap())
}

/// Plays `seq` from `t0` for `steps` steps at 60 Hz (notify windows as a montage's first and following updates).
fn play(a: &mut Animator, seq: &str, t0: f32, steps: usize) -> f32 {
    let names = [seq];
    let dt = 1.0 / 60.0;
    let mut t = t0;
    for i in 0..steps {
        let from = if i == 0 { -1.0 } else { t };
        t += dt;
        let body = BodySlot { samples: [(seq, 1.0), (seq, 0.0)], blend_with: None, notify_sequences: &names, time: t, weight: 1.0, active: true, notify_window: (from, t) };
        a.step(&Frame { dt, body, component_to_world: Trs::IDENTITY }, &NoIkWorld);
    }
    t
}

#[test]
fn reset_and_pre_roll_runs_the_extra_steps() {
    let Some(mut a) = animator("n_ex001") else { return };
    play(&mut a, "0001", 0.0, 60);
    let before = a.bonamik().unwrap().stats();
    // 1641: ResetAndPreRoll m_ResetPose true, m_PreRoll 6 at 0.0001 s; the next evaluations: a hard reset and 6 steps
    play(&mut a, "1641", 0.0, 4);
    let after = a.bonamik().unwrap().stats();
    println!("Donald 1641: resets {} -> {}, steps {} -> {}", before.resets, after.resets, before.steps, after.steps);
    assert!(after.resets > before.resets, "the reset ran");
    assert!(after.steps - before.steps >= 6, "the pre-roll's 6 steps (plus the normal ones): {}", after.steps - before.steps);
    let c = a.bonamik().unwrap().core();
    assert!(c.f33ea == 0 && c.f33ec == 0, "the pre-roll request is consumed");
}

#[test]
fn swim_and_reaction_clip_states() {
    let Some(mut a) = animator("n_ex001") else { return };
    play(&mut a, "0001", 0.0, 10);
    let groups = a.bonamik().unwrap().core().f410;
    let initial: Vec<u8> = a.bonamik().unwrap().core().f924[..groups].to_vec();
    // 1510 (EnableGroundCollision 0 .. 1.77 s, m_Enable not serialized = false): ground collision off for every group
    let t = play(&mut a, "1510", 0.0, 10);
    assert!(a.bonamik().unwrap().core().f924[..groups].iter().all(|g| *g == 0), "begin: m_Enable false on all groups");
    let _ = t;
    play(&mut a, "0001", 0.0, 3);
    assert!(a.bonamik().unwrap().core().f924[..groups].iter().all(|g| *g == 1), "end: !m_Enable on all groups");
    println!("Donald ground flags: asset {initial:?}, during 1510 all 0, after all 1");
    // 0085 (swim): SpeedRate 0.2 while it plays (begin blend 0), back to the saved rate at its end
    play(&mut a, "0085", 0.0, 10);
    let during = a.bonamik().unwrap().speed_rate();
    let c = a.bonamik().unwrap().core();
    // ScaleGroupParams (all groups: inner cone 0.05, local force 0.2; Vest: 0.1 / 0.2; outer / wind / mass 1)
    let vest = c.f4d8[..c.f410].iter().position(|n| n.eq_ignore_ascii_case("Vest"));
    let (inner, local) = (c.fb94[0], c.fd14[0]);
    let vest_inner = vest.map(|g| c.fb94[g]);
    play(&mut a, "0001", 0.0, 3);
    let after = a.bonamik().unwrap().speed_rate();
    let c = a.bonamik().unwrap().core();
    println!("Donald 0085: speed rate {during} -> {after}; group 0 inner cone x{inner} local force x{local}, Vest inner x{vest_inner:?}; after: x{} x{}; unported {:?}; warnings {:?}", c.fb94[0], c.fd14[0], a.unported_cloth, a.warnings);
    assert!((during - 0.2).abs() < 1.0e-6 && (after - 1.0).abs() < 1.0e-6);
    assert!(!a.warnings.iter().any(|w| w.contains("ChangeGroupParams") || w.contains("CharaWind")), "{:?}", a.warnings);
    assert!(a.unported_cloth.is_empty(), "{:?}", a.unported_cloth);
    assert!(c.fb94[0] == 1.0 && c.fd14[0] == 1.0, "restored to 1 at the end");
    assert!(inner < 1.0 && local < 1.0, "scaled while 0085 plays");
    // Goofy 0086: Bonamik_Weight on Arm (0.7, time 0): target 1 - 0.7, state 2
    let Some(mut g) = animator("n_ex002") else { return };
    play(&mut g, "0001", 0.0, 10);
    play(&mut g, "0086", 0.0, 5);
    let c = g.bonamik().unwrap().core();
    println!("Goofy groups {:?}; unported {:?}", &c.f4d8[..c.f410], g.unported_cloth);
    assert!(g.unported_cloth.is_empty());
    match c.f4d8[..c.f410].iter().position(|n| n.eq_ignore_ascii_case("Arm")) {
        Some(arm) => assert!((c.f3638[arm] - 0.3).abs() < 1.0e-6, "Arm target {}", c.f3638[arm]),
        // a name that matches no group of the asset is ignored, as in the game
        None => println!("Goofy 0086: no group Arm in the default asset: the Weight notify changes nothing"),
    }
}

#[test]
fn ground_clamps_the_cloth() {
    for npc in ["n_ex001", "n_ex002"] {
        let Some(mut a) = animator(npc) else { return };
        let enabled = a.bonamik().unwrap().core().f65d;
        play(&mut a, "0001", 0.0, 90);
        // the dynamic particles (f276 == 0) of the groups with ground collision on (core+0x924)
        let lowest_dynamic = |a: &Animator| {
            let c = a.bonamik().unwrap().core();
            c.f3778.f0.iter().filter(|p| p.f276 == 0 && c.f924.get(p.f2b8).copied() == Some(1)).map(|p| p.f0[2]).fold(f32::INFINITY, f32::min)
        };
        let lowest = lowest_dynamic(&a);
        // a floor 8 cm above the lowest of them
        let floor = lowest + 8.0;
        a.set_bonamik_ground(Some((Vec3::new(0.0, 0.0, floor), Vec3::Z)));
        play(&mut a, "0001", 1.5, 90);
        let low2 = lowest_dynamic(&a);
        println!("{npc}: asset enableGroundCollision {enabled}; lowest particle {lowest:.2} -> {low2:.2} with a floor at {floor:.2}");
        if enabled != 0 {
            assert!(low2 > lowest + 1.0, "{npc}: the floor pushes the cloth up");
        }
    }
}
