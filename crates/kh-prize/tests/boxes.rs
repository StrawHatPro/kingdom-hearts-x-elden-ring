//! (loot) The item boxes against docs/sora-prize-spec.md section 11 (sheet sora-prizes rows box.* / table.*).

use kh_core::test_world::TestWorld;
use kh_core::Vec3;
use kh_prize::params::P;
use kh_prize::{item_chance, lucky_rate, BoxKind, Getter, PrizeClass, PrizeEvent, Prizes, SpawnMods, State};

const DT: f32 = 1.0 / 60.0;

/// 11.1: the box of an item is chosen by the high byte of its item id (0x141473110).
#[test]
fn the_box_kind_is_the_items_category() {
    assert_eq!(BoxKind::of_item(0x101), Some(BoxKind::Heal), "Potion");
    assert_eq!(BoxKind::of_item(0x10B), Some(BoxKind::Heal), "Hi-Ether");
    assert_eq!(BoxKind::of_item(0x201), Some(BoxKind::Heal), "Tent (a camp item)");
    assert_eq!(BoxKind::of_item(0x701), Some(BoxKind::Material), "Blazing Shard");
    assert_eq!(BoxKind::of_item(0x73C), Some(BoxKind::Material), "Illusory Crystal");
    assert_eq!(BoxKind::of_item(0x801), Some(BoxKind::Foodstuff));
    assert_eq!(BoxKind::of_item(0x404), Some(BoxKind::Rare), "armour");
    assert_eq!(BoxKind::of_item(0x50C), Some(BoxKind::Rare), "an accessory");
    assert_eq!(BoxKind::of_item(0x301), Some(BoxKind::Rare), "a Keyblade");
    assert_eq!(BoxKind::of_item(0), None);
}

/// 11.3: Luck Up multiplies a rate by 1 + 0.3 n, rounded; the roll is uniform over 1 ..= 99.
#[test]
fn luck_multiplies_the_rate_and_the_roll_is_out_of_99() {
    assert_eq!(lucky_rate(8, 0), 8);
    assert_eq!(lucky_rate(8, 1), 10, "8 x 1.3 = 10.4");
    assert_eq!(lucky_rate(8, 2), 13, "8 x 1.6 = 12.8");
    assert_eq!(lucky_rate(8, 3), 15, "8 x 1.9 = 15.2");
    assert_eq!(lucky_rate(4, 1), 5, "4 x 1.3 = 5.2");
    assert_eq!(lucky_rate(50, 5), 125, "above 100: always");
    assert!((item_chance(8, 0) - 8.0 / 99.0).abs() < 1.0e-6);
    assert_eq!(item_chance(99, 0), 1.0);
    assert_eq!(item_chance(100, 0), 1.0);
    assert_eq!(item_chance(50, 5), 1.0);
    // Measured: 20000 kills of a table (Potion 8 %, Blazing Shard 8 %, Frost Shard 4 %), each slot by itself.
    for (luck, expect) in [(0u32, [8.0f32 / 99.0, 8.0 / 99.0, 4.0 / 99.0]), (3, [15.0 / 99.0, 15.0 / 99.0, 8.0 / 99.0])] {
        let mut prizes = Prizes::new(11 + luck);
        let mut got = [0u32; 3];
        let (mut none, mut several) = (0, 0);
        let kills = 20_000;
        for _ in 0..kills {
            let made = prizes.drop_items(Vec3::ZERO, &[(0x101, 8), (0x701, 8), (0x705, 4)], luck, false);
            match made.len() {
                0 => none += 1,
                1 => {}
                _ => several += 1,
            }
            for id in made {
                match prizes.get(id).unwrap().class {
                    PrizeClass::Box(BoxKind::Heal, 0x101) => got[0] += 1,
                    PrizeClass::Box(BoxKind::Material, 0x701) => got[1] += 1,
                    PrizeClass::Box(BoxKind::Material, 0x705) => got[2] += 1,
                    other => panic!("{other:?}"),
                }
            }
            prizes.clear();
        }
        for k in 0..3 {
            let rate = got[k] as f32 / kills as f32;
            assert!((rate - expect[k]).abs() < 0.008, "luck {luck} slot {k}: {rate} against {}", expect[k]);
        }
        // independent slots: most kills drop nothing, a few drop more than one item
        assert!(none > kills / 2 && several > 0, "luck {luck}: none {none}, several {several}");
    }
    // a rate of 100 always drops, a rate of 0 or an empty slot never
    let mut prizes = Prizes::new(3);
    for _ in 0..500 {
        assert_eq!(prizes.drop_items(Vec3::ZERO, &[(0x101, 100), (0x701, 0), (0, 100)], 0, false).len(), 1);
    }
}

/// 11.2: a box is thrown mostly upward (600 .. 1200 cm/s up, at most 300 sideways), keeps its sideways speed
/// (damping 0.98 against an orb's 0.955), can be collected from its first frame and lives 180 s.
#[test]
fn a_box_is_thrown_up_can_be_taken_at_once_and_lives_three_minutes() {
    let world = TestWorld::floor();
    let mut prizes = Prizes::new(5);
    let nobody = Getter { point: Vec3::new(1.0e6, 0.0, 91.0), can_take: true, radius: Some(300.0) };
    for _ in 0..200 {
        let id = prizes.drop_item_box(Vec3::new(0.0, 0.0, 150.0), 0x101, false).unwrap();
        let p = *prizes.get(id).unwrap();
        assert!(p.collectible(), "from its first frame (0x141473590)");
        assert!((P.box_speed_z[0]..=P.box_speed_z[1]).contains(&p.velocity.z), "{:?}", p.velocity);
        assert!(p.velocity.x.abs() <= P.box_speed_xy && p.velocity.y.abs() <= P.box_speed_xy);
        assert_eq!(p.time_left(), Some(P.life_box_living_time));
    }
    prizes.clear();
    // one box and one orb with the same sideways speed: after half a second the box has kept more of it
    let b = prizes.spawn(Vec3::new(0.0, 0.0, 5000.0), PrizeClass::Box(BoxKind::Heal, 0x101), Vec3::new(300.0, 0.0, 0.0), &SpawnMods::default(), false, 0);
    let o = prizes.spawn(Vec3::new(0.0, 500.0, 5000.0), PrizeClass::Orb(kh_core::status::PrizeKind::HpSmall), Vec3::new(300.0, 0.0, 0.0), &SpawnMods::default(), false, 0);
    for _ in 0..30 {
        prizes.step(DT, &world, &nobody);
    }
    let (vb, vo) = (prizes.get(b).unwrap().velocity.x, prizes.get(o).unwrap().velocity.x);
    assert!((vb - 300.0 * 0.98f32.powi(30)).abs() < 1.0 && (vo - 300.0 * 0.955f32.powi(30)).abs() < 1.0, "{vb} {vo}");
    // it lies for 180 s, then vanishes
    prizes.clear();
    let id = prizes.drop_item_box(Vec3::new(0.0, 0.0, 150.0), 0x701, false).unwrap();
    let mut gone_at = None;
    for frame in 0..(185 * 60) {
        prizes.step(DT, &world, &nobody);
        if prizes.get(id).is_none() {
            gone_at = Some(frame as f32 * DT);
            break;
        }
    }
    let gone_at = gone_at.expect("it vanishes");
    assert!((gone_at - 180.0).abs() < 0.2, "{gone_at}");
    assert!(prizes.take_events().iter().any(|e| matches!(e, PrizeEvent::Expired { class: PrizeClass::Box(BoxKind::Material, 0x701), .. })));
}

/// 11.4: THE ARC. A box lying on the floor near the getter leaves the straight line upward on its way
/// in, takes clamp(distance / 1200, 0.5, 1.0) s, plays its get sound 0.2 s before the end, turns up to
/// 30 degrees, shrinks at the very end and is collected at the getter.
#[test]
fn a_box_flies_an_arc_to_the_getter() {
    let world = TestWorld::floor();
    let getter = Getter { point: Vec3::new(0.0, 0.0, 91.0), can_take: true, radius: Some(300.0) };
    for (distance, time) in [(250.0f32, 0.5f32), (290.0, 0.5)] {
        let mut prizes = Prizes::new(9);
        // lying still, on the floor (its sphere is 26 cm)
        let id = prizes.spawn(Vec3::new(distance, 0.0, 26.0), PrizeClass::Box(BoxKind::Rare, 0x404), Vec3::ZERO, &SpawnMods::default(), false, 0);
        prizes.take_events();
        let (mut highest, mut frames, mut sound_at, mut widest_yaw, mut smallest) = (0.0f32, 0, None, 0.0f32, 1.0f32);
        let mut nearest = f32::MAX;
        let mut collected = false;
        for frame in 0..120 {
            prizes.step(DT, &world, &getter);
            if let Some(p) = prizes.get(id) {
                assert_eq!(p.state, State::Get, "in reach: it flies from the first step");
                // how far above the straight line from where it lay to him it is (the line's height at its distance)
                let line = 91.0 + (26.0 - 91.0) * (p.position.truncate().length() / distance).clamp(0.0, 1.0);
                highest = highest.max(p.position.z - line);
                widest_yaw = widest_yaw.max(p.yaw);
                smallest = smallest.min(p.scale);
                nearest = nearest.min(p.position.distance(getter.point));
            }
            for e in prizes.take_events() {
                match e {
                    PrizeEvent::GetSound { .. } => sound_at = Some(frame),
                    PrizeEvent::Collected { class, position, .. } => {
                        assert_eq!(class, PrizeClass::Box(BoxKind::Rare, 0x404));
                        assert_eq!(position, getter.point);
                        collected = true;
                        frames = frame;
                    }
                    _ => {}
                }
            }
            if collected {
                break;
            }
        }
        assert!(collected, "distance {distance}");
        let took = frames as f32 * DT;
        assert!((took - time).abs() < 0.05, "distance {distance}: {took} s");
        // it leaves the straight line upward (it sets off 600 cm "above" him) and curves back in
        assert!(highest > 15.0, "distance {distance}: {highest} cm above the straight line at most");
        println!("distance {distance}: at most {highest:.1} cm above the straight line, {took:.3} s");
        assert!(nearest < 40.0, "it comes in to him: {nearest}");
        let sound = sound_at.expect("the get sound") as f32 * DT;
        assert!((took - sound - P.get_sound_at).abs() < 0.05, "sound at {sound}, arrival {took}");
        assert!(widest_yaw > 25.0 && widest_yaw <= 30.0, "{widest_yaw}");
        assert!(smallest < 0.5, "{smallest}");
    }
    // far away (20 m with a Master Treasure Magnet): the flight takes the full second
    let mut prizes = Prizes::new(9);
    let unlimited = Getter { radius: None, ..getter };
    let id = prizes.spawn(Vec3::new(2000.0, 0.0, 26.0), PrizeClass::Box(BoxKind::Heal, 0x101), Vec3::ZERO, &SpawnMods::default(), false, 0);
    let mut frames = 0;
    while prizes.get(id).is_some() && frames < 200 {
        prizes.step(DT, &world, &unlimited);
        frames += 1;
    }
    assert!((frames as f32 * DT - 1.0).abs() < 0.05, "{frames}");
    // out of reach: it stays where it lies
    let mut prizes = Prizes::new(9);
    let id = prizes.spawn(Vec3::new(400.0, 0.0, 26.0), PrizeClass::Box(BoxKind::Heal, 0x101), Vec3::ZERO, &SpawnMods::default(), false, 0);
    for _ in 0..120 {
        prizes.step(DT, &world, &getter);
    }
    assert_eq!(prizes.get(id).unwrap().state, State::Appear);
}
