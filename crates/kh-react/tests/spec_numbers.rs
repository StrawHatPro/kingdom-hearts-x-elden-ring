//! The numbers of docs/sora-enemy-reaction-spec.md section 4, reproduced by stepping the crate at
//! 60 Hz in a flat world (`cargo test -p kh-react -- --nocapture` prints the traces the spec quotes).

use glam::Vec3;
use kh_core::host::HitRecord;
use kh_react::*;

use ETresDamageKind as K;

fn medium() -> EnemyReaction {
    EnemyReaction::new(Body::of(BodyClass::Medium))
}

fn request(kind: K) -> ReactionRequest {
    ReactionRequest { attacker_valid: true, attacker_radius: 34.0, ..ReactionRequest::new(kind, Vec3::X) }
}

/// Steps until the reaction finishes (or `max` steps): every output.
fn play(r: &mut EnemyReaction, world: &dyn ReactWorld, max: usize) -> Vec<ReactOutput> {
    let mut out = Vec::new();
    for _ in 0..max {
        let o = r.step(world);
        out.push(o);
        if o.finished {
            break;
        }
    }
    out
}

fn time(step: usize) -> f32 {
    (step + 1) as f32 / 60.0
}

fn first(out: &[ReactOutput], phase: Phase) -> f32 {
    time(out.iter().position(|o| o.phase == phase).unwrap_or_else(|| panic!("no {phase:?}")))
}

fn apex(out: &[ReactOutput]) -> (f32, f32) {
    let (i, o) = out.iter().enumerate().max_by(|a, b| a.1.position.z.total_cmp(&b.1.position.z)).unwrap();
    (o.position.z, time(i))
}

fn near(a: f32, b: f32, tolerance: f32) -> bool {
    (a - b).abs() <= tolerance
}

#[test]
fn a_ground_flinch_slides_back_and_lasts_its_clip() {
    let (world, mut r) = (FlatWorld::default(), medium());
    assert_eq!(r.hit(&request(K::Small), Pose { position: Vec3::ZERO, yaw: 180.0, grounded: true }), HitOutcome::Started { kind: K::Small, downgraded: false });
    let out = play(&mut r, &world, 400);
    let last = out.last().unwrap();
    println!("ground SMALL: ends t={:.3} s after {:.1} cm", time(out.len() - 1), last.position.x);
    // 550 cm/s braked x0.95 per frame: 183.3 cm in the limit, 174.9 by the end of the 1.0 s clip.
    assert!(near(last.position.x, 174.9, 1.0) && last.position.z == 0.0);
    assert!(near(time(out.len() - 1), 1.0 + 1.0 / 60.0, 0.02));
    assert!(out.iter().take(out.len() - 1).all(|o| o.phase == Phase::FlinchGround && o.suspended && o.grounded));
    assert!(last.finished && !last.suspended && last.phase == Phase::None);
    // The victim was facing the attacker (at -X): attacker in front, facing kept.
    assert_eq!((out[0].direction_index, near(last.yaw.abs(), 180.0, 0.01)), (0, true));
}

#[test]
fn the_launcher_tosses_383_cm_and_the_victim_is_down_for_the_clip() {
    // Sora's launcher rows (KB_LAUNCHSPIN, RUN_SLOPE, FF_SONIC_2 ...) are TOSS at power scale 1.
    let (world, mut r) = (FlatWorld::default(), medium());
    r.hit(&request(K::Toss), Pose { position: Vec3::ZERO, yaw: 0.0, grounded: true });
    let out = play(&mut r, &world, 600);
    let (height, at) = apex(&out);
    let (hover, fall, down) = (first(&out, Phase::LaunchedHover), first(&out, Phase::LaunchedFalling), first(&out, Phase::Down));
    println!("TOSS: apex {height:.1} cm at {at:.3} s, hover from {hover:.3}, falling from {fall:.3}, down at {down:.3}, ends {:.3}", time(out.len() - 1));
    // 1500 cm/s against 3 x 980: 1500^2 / (2 x 2940) = 382.7 cm after 0.51 s.
    assert!(near(height, 382.7, 1.0) && near(at, 0.51, 0.04));
    // 0.2 s of hover with gravity off, then the fall at 1 g (0.88 s for 383 cm).
    assert!(near(fall - hover, 0.2, 0.02));
    assert!(near(down - fall, (2.0f32 * 378.0 / 980.0).sqrt(), 0.05));
    // Straight up: no horizontal travel. Hang time (off the ground): about 1.6 s.
    assert!(out.iter().all(|o| o.position.truncate().length() < 0.01));
    assert!(near(down, 1.62, 0.05));
    // Down + getting up is one clip (1.7 s median); then control returns.
    assert!(near(time(out.len() - 1) - down, 1.7, 0.03) && out.last().unwrap().finished);
}

#[test]
fn a_launch_reaches_1063_cm() {
    let (world, mut r) = (FlatWorld::default(), medium());
    r.hit(&request(K::Launch), Pose { position: Vec3::ZERO, yaw: 0.0, grounded: true });
    let out = play(&mut r, &world, 600);
    let (height, at) = apex(&out);
    println!("LAUNCH: apex {height:.1} cm at {at:.3} s, down at {:.3}", first(&out, Phase::Down));
    assert!(near(height, 2500.0 * 2500.0 / (2.0 * 2940.0), 1.0) && near(at, 0.85, 0.04));
    assert!(near(first(&out, Phase::Down), 2.47, 0.05));
    // The row's effect time replaces the power when set.
    let mut r = medium();
    r.hit(&ReactionRequest { effect_time: 1000.0, ..request(K::Launch) }, Pose { position: Vec3::ZERO, yaw: 0.0, grounded: true });
    assert!(near(apex(&play(&mut r, &world, 600)).0, 1000.0 * 1000.0 / (2.0 * 2940.0), 1.0));
}

#[test]
fn an_air_combo_keeps_a_medium_enemy_up() {
    // Launcher, then five air hits (SMALL, power scale 1: KB_AERIALSWEEP, KB_AERIALDIVE ...) 0.35 s
    // apart, the first as it passes its apex.
    let (world, mut r) = (FlatWorld::default(), medium());
    r.hit(&request(K::Toss), Pose { position: Vec3::ZERO, yaw: 180.0, grounded: true });
    let mut heights = Vec::new();
    let (mut step, mut landed_at) = (0usize, None);
    let hit_steps: Vec<usize> = (0..5).map(|i| 33 + i * 21).collect();
    for _ in 0..600 {
        if hit_steps.contains(&step) {
            let o = r.hit(&request(K::Small), Pose { position: r.position(), yaw: 180.0, grounded: false });
            assert_eq!(o, HitOutcome::Started { kind: K::Small, downgraded: false });
            heights.push(r.position().z);
        }
        let o = r.step(&world);
        step += 1;
        if step > 40 && step < 33 + 5 * 21 {
            assert!(!o.grounded && o.position.z > 370.0, "dropped during the combo at step {step}: {o:?}");
        }
        if o.landed {
            landed_at = Some(step);
        }
        if o.finished {
            break;
        }
    }
    println!("air combo: height at each hit {heights:?}, lands at {:.3} s, ends {:.3} s", landed_at.unwrap() as f32 / 60.0, step as f32 / 60.0);
    // Every hit stops the fall, lifts 200 cm/s (braked) with gravity off: the enemy GAINS height
    // from hit to hit (about 40 cm each at this rhythm) instead of dropping.
    assert!(heights.windows(2).all(|w| w[1] > w[0] + 25.0 && w[1] < w[0] + 55.0), "{heights:?}");
    // It was pushed away from the attacker by each hit (about 130 cm per hit).
    assert!(r.position().x > 5.0 * 100.0);
    // After the last hit it drops on its FEET (the Land clip), not down: an air flinch ends standing.
    assert!(landed_at.unwrap() > 33 + 4 * 21 + 40);
}

#[test]
fn one_air_hit_hangs_for_0_45_s_and_rises_51_cm() {
    let (world, mut r) = (FlatWorld::default(), medium());
    r.hit(&request(K::Small), Pose { position: Vec3::new(0.0, 0.0, 300.0), yaw: 180.0, grounded: false });
    let out = play(&mut r, &world, 600);
    let (height, at) = apex(&out);
    // Gravity is off until the horizontal speed (550 x 0.95^n) is at or below 150: 26 frames.
    let hang = out.iter().skip(2).position(|o| o.velocity.z < 0.0).unwrap() + 2;
    println!("air SMALL: apex +{:.1} cm at {at:.3} s, gravity back after {:.3} s, lands {:.3}, ends {:.3}", height - 300.0, time(hang), first(&out, Phase::LandFeet), time(out.len() - 1));
    assert!(near(height - 300.0, 51.0, 2.0));
    assert!(near(time(hang), 0.5, 0.05));
    assert_eq!(out.last().map(|o| (o.finished, o.grounded)), Some((true, true)));
    assert!(out.iter().any(|o| o.phase == Phase::Falling) && out.iter().any(|o| o.phase == Phase::LandFeet));
}

#[test]
fn the_finisher_blows_a_ground_victim_278_cm_and_an_airborne_one_663_cm() {
    let world = FlatWorld::default();
    // On the ground: 1500 cm/s x 0.91 per frame, no lift: a knock-down slide.
    let mut r = medium();
    r.hit(&request(K::Blow), Pose { position: Vec3::ZERO, yaw: 180.0, grounded: true });
    let out = play(&mut r, &world, 600);
    println!("ground BLOW: {:.1} cm, down at {:.3} s, ends {:.3} s", out.last().unwrap().position.x, first(&out, Phase::Down), time(out.len() - 1));
    assert!(near(out.last().unwrap().position.x, 1500.0 / 60.0 / (1.0 - 0.91), 1.5));
    assert!(apex(&out).0 < 0.01 && first(&out, Phase::Down) < 0.2);
    assert!(near(time(out.len() - 1), 0.13 + 1.667, 0.05));
    // In the air (the end of an air combo): 2000 cm/s x 0.95 per frame and 300 cm/s up, gravity off
    // until the horizontal speed is at or below 250.
    let mut r = medium();
    r.hit(&request(K::Blow), Pose { position: Vec3::new(0.0, 0.0, 300.0), yaw: 180.0, grounded: false });
    let out = play(&mut r, &world, 600);
    let (height, at) = apex(&out);
    println!("air BLOW from 300 cm: {:.1} cm away, apex +{:.1} cm at {at:.3} s, falling pose from {:.3}, down at {:.3}, ends {:.3}", out.last().unwrap().position.x, height - 300.0, first(&out, Phase::BlownFalling), first(&out, Phase::Down), time(out.len() - 1));
    assert!(near(out.last().unwrap().position.x, 663.0, 3.0) && near(height - 300.0, 119.5, 3.0));
    assert!(out.iter().any(|o| o.phase == Phase::BlownFalling));
    // The row's power scale scales the distance (LM_AERIALFINISH2: 0.75).
    let mut r = medium();
    r.hit(&ReactionRequest { power_scale: 0.75, ..request(K::Blow) }, Pose { position: Vec3::ZERO, yaw: 180.0, grounded: true });
    assert!(near(play(&mut r, &world, 600).last().unwrap().position.x, 0.75 * 277.8, 1.5));
}

#[test]
fn a_blown_victim_sticks_to_a_wall() {
    // A wall 150 cm behind the victim, facing it.
    let world = FlatWorld { floor_z: 0.0, walls: vec![(Vec3::new(150.0, 0.0, 0.0), Vec3::NEG_X)] };
    let mut r = medium();
    r.hit(&request(K::Blow), Pose { position: Vec3::new(0.0, 0.0, 200.0), yaw: 180.0, grounded: false });
    let out = play(&mut r, &world, 600);
    let hit = out.iter().position(|o| o.wall_hit).expect("wall hit");
    println!("wall: hit at {:.3} s, x {:.1}, falls from {:.3}, down at {:.3}, ends {:.3}", time(hit), out[hit].position.x, first(&out, Phase::BlownFalling), first(&out, Phase::Down), time(out.len() - 1));
    assert!(out[hit].position.x <= 150.0 && out[hit].phase == Phase::WallHit && out[hit].velocity == Vec3::ZERO);
    // Stuck with gravity off for the wall clip (0.5 s), then it drops and lies for the wall-down clip.
    let stuck = out.iter().filter(|o| o.phase == Phase::WallHit).count();
    assert!(near(stuck as f32 / 60.0, 0.5, 0.03));
    assert!(out[hit..hit + stuck].iter().all(|o| near(o.position.z, out[hit].position.z, 0.01)));
    assert!(out.last().unwrap().grounded && out.last().unwrap().finished && out.iter().all(|o| o.position.x <= 150.2));
    // Too slow (a ground flinch at 550 cm/s is not a blow) or too shallow: no wall reaction.
    let mut r = medium();
    r.hit(&request(K::Small), Pose { position: Vec3::new(100.0, 0.0, 0.0), yaw: 180.0, grounded: true });
    assert!(play(&mut r, &world, 600).iter().all(|o| !o.wall_hit && o.position.x <= 150.2));
    let grazing = FlatWorld { floor_z: 0.0, walls: vec![(Vec3::new(150.0, 0.0, 0.0), Vec3::new(-0.3, -0.954, 0.0).normalize())] };
    let mut r = medium();
    r.hit(&request(K::Blow), Pose { position: Vec3::new(0.0, 0.0, 200.0), yaw: 180.0, grounded: false });
    assert!(play(&mut r, &grazing, 600).iter().all(|o| !o.wall_hit));
}

#[test]
fn a_spin_carries_the_victim_470_cm_along_its_root_motion() {
    let world = FlatWorld::default();
    for kind in [K::Sidespin, K::Verticalroll] {
        let mut r = medium();
        r.hit(&request(kind), Pose { position: Vec3::ZERO, yaw: 180.0, grounded: true });
        let out = play(&mut r, &world, 600);
        println!("{kind:?}: {:.1} cm, down at {:.3} s, ends {:.3} s", out.last().unwrap().position.x, first(&out, Phase::Down), time(out.len() - 1));
        assert!(near(out.last().unwrap().position.x, 470.0, 5.0) && near(first(&out, Phase::Down), 0.9, 0.05));
    }
    // An enemy without the clip takes it as a BLOW.
    let mut body = Body::of(BodyClass::Medium);
    body.clips.spin = None;
    let mut r = EnemyReaction::new(body);
    assert_eq!(r.hit(&request(K::Sidespin), Pose { position: Vec3::ZERO, yaw: 180.0, grounded: true }), HitOutcome::Started { kind: K::Sidespin, downgraded: false });
    assert_eq!(r.reaction_state(), K::Blow.value() as u8);
}

#[test]
fn a_beat_spikes_an_airborne_victim_and_does_nothing_visible_on_the_ground() {
    let world = FlatWorld::default();
    let mut r = medium();
    r.hit(&request(K::Beat), Pose { position: Vec3::new(0.0, 0.0, 300.0), yaw: 180.0, grounded: false });
    let out = play(&mut r, &world, 600);
    println!("BEAT from 300 cm: lands after {:.3} s, {:.1} cm away", time(out.len() - 1), out.last().unwrap().position.x);
    // 1500 cm/s down: 300 cm in 0.19 s; spaced out by 0.7071 x (40 + 34) cm; no slam clip: over at the landing.
    assert!(near(time(out.len() - 1), 0.21, 0.03) && near(out.last().unwrap().position.x, 0.7071 * 74.0, 6.0));
    assert!(out.last().unwrap().finished && out.last().unwrap().landed);
    let mut r = medium();
    r.hit(&request(K::Beat), Pose { position: Vec3::ZERO, yaw: 180.0, grounded: true });
    assert!(play(&mut r, &world, 600).len() <= 3);
}

#[test]
fn a_strong_body_reacts_only_to_reaction_power_once_broken() {
    let mut r = EnemyReaction::new(Body::of(BodyClass::Heavy));
    let pose = Pose { position: Vec3::ZERO, yaw: 180.0, grounded: true };
    assert_eq!((r.target_body().reaction, r.endurance()), (BodyReaction::Strong, 5));
    // What the classification makes of the endurance (kh_core::combat::classify, body type STRONG).
    let result = |left: i32, rp: i32| {
        if left > 0 {
            if rp > 1 {
                ReactionResult::StrongBodyDamage2
            } else {
                ReactionResult::StrongBodyDamage1
            }
        } else if rp > 0 {
            ReactionResult::StrongBodyDamage4
        } else {
            ReactionResult::StrongBodyDamage3
        }
    };
    let hit = |r: &mut EnemyReaction, kind: K, rp: i32| {
        let left = r.take_reaction_power(rp);
        r.hit(&ReactionRequest { victim_result: result(left, rp), ..request(kind) }, pose)
    };
    // Basic hits (reaction power 0) never flinch it.
    assert_eq!(hit(&mut r, K::Small, 0), HitOutcome::NoReaction);
    // Finishers carry 1: four of them wear it down without a reaction, the fifth breaks it and lands in full.
    for _ in 0..4 {
        assert_eq!(hit(&mut r, K::Blow, 1), HitOutcome::NoReaction);
    }
    assert_eq!(hit(&mut r, K::Blow, 1), HitOutcome::Started { kind: K::Blow, downgraded: false });
    assert_eq!(r.endurance(), 0);
    // Broken: a basic hit still does nothing, a big kind without power is a flinch, power lands in full.
    r.release();
    assert_eq!(hit(&mut r, K::Small, 0), HitOutcome::NoReaction);
    assert_eq!(hit(&mut r, K::Toss, 0), HitOutcome::Started { kind: K::Small, downgraded: true });
    r.release();
    assert_eq!(hit(&mut r, K::Toss, 1), HitOutcome::Started { kind: K::Toss, downgraded: false });
    r.release();
    // A counter (reaction power 5) breaks a fresh body of 5 at once.
    let mut fresh = EnemyReaction::new(Body::of(BodyClass::Heavy));
    assert_eq!(hit(&mut fresh, K::Blow, 5), HitOutcome::Started { kind: K::Blow, downgraded: false });
    // 10 s after the last hit with power the broken body is whole again.
    let world = FlatWorld::default();
    for _ in 0..598 {
        r.step(&world);
    }
    assert_eq!(r.endurance(), 0);
    for _ in 0..4 {
        r.step(&world);
    }
    assert_eq!(r.endurance(), 5);
}

#[test]
fn revenge_points_raise_the_flag_at_the_limit_and_decay_after_3_s() {
    let mut body = Body::of(BodyClass::Medium);
    body.revenge_limit = 6.5;
    let (world, mut r) = (FlatWorld::default(), EnemyReaction::new(body));
    let pose = Pose { position: Vec3::ZERO, yaw: 180.0, grounded: true };
    // A ground combo of Sora: three basic hits worth 1 point each and a finisher worth 0.5.
    let combo = |r: &mut EnemyReaction| {
        for points in [1.0, 1.0, 1.0, 0.5] {
            r.hit(&ReactionRequest { revenge_point: points, ..request(K::Small) }, pose);
            r.release();
        }
    };
    combo(&mut r);
    assert!(near(r.revenge_value(), 3.5, 1.0e-4) && !r.take_revenge());
    // The second combo crosses 6.5 on its third hit.
    combo(&mut r);
    assert!(near(r.revenge_value(), 7.0, 1.0e-4) && r.take_revenge() && !r.take_revenge());
    // Left alone: held 3 s, then 5 points per second.
    for _ in 0..180 {
        r.step(&world);
    }
    assert!(near(r.revenge_value(), 7.0, 0.1));
    for _ in 0..60 {
        r.step(&world);
    }
    assert!(near(r.revenge_value(), 2.0, 0.2));
    r.clear_revenge();
    assert_eq!(r.revenge_value(), 0.0);
    // No limit (ordinary heartless): no revenge, ever.
    let mut plain = medium();
    for _ in 0..40 {
        plain.hit(&ReactionRequest { revenge_point: 1.0, ..request(K::Small) }, pose);
    }
    assert!(!plain.take_revenge() && plain.revenge_value() == 0.0);
}

#[test]
fn guards_kills_and_unported_kinds_start_nothing() {
    let pose = Pose { position: Vec3::ZERO, yaw: 0.0, grounded: true };
    let mut r = medium();
    assert_eq!(r.hit(&ReactionRequest { victim_result: ReactionResult::Guard, ..request(K::Small) }, pose), HitOutcome::HostState(ReactionResult::Guard));
    assert_eq!(r.hit(&ReactionRequest { victim_result: ReactionResult::NoReaction, ..request(K::Blow) }, pose), HitOutcome::NoReaction);
    assert_eq!(r.hit(&request(K::Magnet), pose), HitOutcome::NotPorted);
    assert_eq!(r.hit(&ReactionRequest { lethal: true, ..request(K::Blow) }, pose), HitOutcome::Dying);
    assert!(!r.is_active() && r.reaction_state() == 0);
    let mut never = EnemyReaction::new(Body { never_reacts: true, ..Body::of(BodyClass::Giant) });
    assert_eq!(never.hit(&request(K::Blow), pose), HitOutcome::NoReaction);
    // An idle reaction answers "not suspended" and leaves the host's position alone.
    let o = r.step(&FlatWorld::default());
    assert!(!o.suspended && !o.finished && o.phase == Phase::None);
}

#[test]
fn a_core_hit_record_feeds_the_reaction_directly() {
    // What `kh_core::combat::hit` delivers for KB_LAUNCHSPIN (TOSS, scale 1) from Sora at the origin.
    let mut hit = HitRecord::empty(kh_core::TargetId(1));
    hit.damage_kind = K::Toss;
    hit.direction = Vec3::new(1.0, 0.0, 0.0);
    hit.info.revenge_point = 1.0;
    hit.attacker = kh_core::host::HitAttacker { position: Vec3::new(-100.0, 0.0, 90.0), yaw: 0.0, radius: 34.0, valid: true };
    let request = ReactionRequest::from_hit(&hit, true, false);
    assert_eq!((request.kind, request.power_scale, request.attacker_valid, request.attacker_radius), (K::Toss, 1.0, true, 34.0));
    let mut r = medium();
    assert_eq!(r.hit(&request, Pose { position: Vec3::ZERO, yaw: 180.0, grounded: true }), HitOutcome::Started { kind: K::Toss, downgraded: false });
    // A hit without a horizontal direction is pushed along the attacker's facing.
    hit.damage_kind = K::Small;
    hit.direction = Vec3::Z;
    hit.attacker.yaw = 90.0;
    let mut r = medium();
    r.hit(&ReactionRequest::from_hit(&hit, true, false), Pose { position: Vec3::ZERO, yaw: 0.0, grounded: true });
    let out = play(&mut r, &FlatWorld::default(), 400);
    assert!(out.last().unwrap().position.y > 150.0 && out.last().unwrap().position.x.abs() < 0.01);
}
