//! The prize orbs against docs/sora-prize-spec.md (sheet sora-prizes). Every expectation names
//! the spec section or the row it comes from.

use kh_core::status::{LoadoutValue, PrizeKind};
use kh_core::test_world::TestWorld;
use kh_core::{Input, Loadout, Pose, Sora, Status, Vec3, SORA_PARAMS};
use kh_prize::params::P;
use kh_prize::{host, BoxKind, Converters, DropTable, Getter, PrizeClass, PrizeEvent, Prizes, SpawnMods, State};

const DT: f32 = 1.0 / 60.0;

fn far() -> Getter {
    Getter { point: Vec3::new(1.0e6, 0.0, 91.0), can_take: true, radius: Some(300.0) }
}

fn sora99() -> Sora {
    let path = format!("{}/../../sheets/port/sora-loadout-99.json", env!("CARGO_MANIFEST_DIR"));
    let json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let mut loadout = Loadout::new("sora-loadout-99");
    for row in json["rows"].as_array().unwrap() {
        let value = match &row[1] {
            serde_json::Value::Number(n) => n.as_f64().map(LoadoutValue::Number),
            serde_json::Value::String(s) => Some(LoadoutValue::Text(s.clone())),
            serde_json::Value::Array(items) => items.iter().map(|i| i.as_str().map(str::to_string)).collect::<Option<Vec<_>>>().map(LoadoutValue::List),
            serde_json::Value::Object(map) => map.iter().map(|(k, v)| v.as_f64().map(|n| (k.clone(), n))).collect::<Option<Vec<_>>>().map(LoadoutValue::Map),
            _ => None,
        };
        loadout.set(row[0].as_str().unwrap(), &value.unwrap()).unwrap();
    }
    loadout.resolve().unwrap();
    Sora::new(SORA_PARAMS, Pose { position: Vec3::new(0.0, 0.0, Sora::origin_height_on_floor(&SORA_PARAMS)), yaw: 0.0 }).with_status(Status::from_loadout(&loadout))
}

/// Spec 3: a table drop throws every orb at 900 cm/s, 50 .. 65 degrees up, all round; the same
/// seed throws the same orbs.
#[test]
fn a_table_drop_is_thrown_in_the_games_cone_and_is_deterministic() {
    let table = DropTable::of_row(P.test_dummy4);
    let mut a = Prizes::new(7);
    assert_eq!(a.drop_table(Vec3::new(0.0, 0.0, 200.0), &table, &SpawnMods::default(), false), 7, "HpL x3 + MunnyL x4");
    let kinds: Vec<_> = a.list().iter().map(|p| p.class).collect();
    assert_eq!(kinds[..3], [PrizeClass::Orb(PrizeKind::HpLarge); 3]);
    assert_eq!(kinds[3..], [PrizeClass::Orb(PrizeKind::Munny10); 4]);
    let mut quadrants = [false; 4];
    for p in a.list() {
        let speed = p.velocity.length();
        let pitch = (p.velocity.z / speed).asin().to_degrees();
        assert!((speed - 900.0).abs() < 0.01 && (49.999..=65.001).contains(&pitch), "speed {speed} pitch {pitch}");
        quadrants[(p.velocity.x > 0.0) as usize * 2 + (p.velocity.y > 0.0) as usize] = true;
    }
    assert!(quadrants.iter().filter(|q| **q).count() >= 3, "all round: {quadrants:?}");
    let mut b = Prizes::new(7);
    b.drop_table(Vec3::new(0.0, 0.0, 200.0), &table, &SpawnMods::default(), false);
    assert_eq!(a.list(), b.list());
    let mut c = Prizes::new(8);
    c.drop_table(Vec3::new(0.0, 0.0, 200.0), &table, &SpawnMods::default(), false);
    assert_ne!(a.list()[0].velocity, c.list()[0].velocity);
}

/// Spec 4: gravity 2.3 x 980, each bounce keeps 0.7 of the speed along the normal, the sound
/// plays for the first four bounces, and from the fourth bounce (at least 1 s old) it rests.
/// It never goes through the floor.
#[test]
fn an_orb_bounces_lower_each_time_and_comes_to_rest_on_the_floor() {
    let world = TestWorld::floor();
    let mut prizes = Prizes::new(1);
    let radius = PrizeClass::Orb(PrizeKind::HpLarge).radius();
    assert_eq!(radius, 26.0);
    prizes.spawn(Vec3::new(0.0, 0.0, 100.0), PrizeClass::Orb(PrizeKind::HpLarge), Vec3::new(300.0, 0.0, 700.0), &SpawnMods::default(), false, 0);
    prizes.take_events();
    let (mut apex, mut apexes, mut rising) = (0.0f32, Vec::new(), true);
    let mut bounce_events = Vec::new();
    let mut lowest = f32::MAX;
    // One step: the first step's gravity (row move.gravity_scale x move.world_gravity_z).
    prizes.step(DT, &world, &far());
    assert!((prizes.list()[0].velocity.z - (700.0 - 2.3 * 980.0 * DT)).abs() < 0.01);
    for step in 1..(6 * 60) {
        prizes.step(DT, &world, &far());
        let p = prizes.list()[0];
        lowest = lowest.min(p.position.z);
        if p.velocity.z > 0.0 {
            rising = true;
            apex = apex.max(p.position.z);
        } else if rising {
            rising = false;
            apexes.push(apex.max(p.position.z));
            apex = 0.0;
        }
        for e in prizes.take_events() {
            if let PrizeEvent::Bounced { count, sound, .. } = e {
                bounce_events.push((step, count, sound));
            }
        }
        if p.resting() {
            break;
        }
    }
    let p = prizes.list()[0];
    assert!(lowest >= radius - 0.01, "no tunnelling: lowest centre {lowest}");
    assert!(p.resting() && p.velocity == Vec3::ZERO && (p.position.z - radius).abs() < 1.0, "rests on the floor: {p:?}");
    assert!(apexes.len() >= 3 && apexes.windows(2).all(|w| w[1] < w[0]), "each bounce is lower: {apexes:?}");
    // First apex: 100 + 700^2 / (2 x 2254) = 208.7 cm; after the first bounce the speed up is
    // 0.7 of the speed down: (0.7^2) of the fall height above the floor contact.
    assert!((apexes[0] - 208.7).abs() < 1.5, "{apexes:?}");
    let fall = apexes[0] - radius;
    assert!(((apexes[1] - radius) - 0.49 * fall).abs() < 3.0, "{apexes:?}");
    // Rows bounce.sound_max / rest.min_bounces: sounds on bounces 1 .. 4 only; rest at the 4th or later.
    assert!(bounce_events.iter().all(|&(_, count, sound)| sound == (count <= 4)));
    assert!(bounce_events.last().unwrap().1 >= 4 && p.age >= 1.0);
    // Row move.damping: the sideways speed fell by 0.955 per frame while it flew.
    assert!(p.position.x > 60.0 && p.position.x < 300.0 / (60.0 * (1.0 - 0.955)) + 1.0, "x {}", p.position.x);
}

/// Spec 4: a wall stops it too (the sweep is the orb's sphere against the host's world).
#[test]
fn an_orb_bounces_off_a_wall() {
    let world = TestWorld::floor().with_box(Vec3::new(200.0, -500.0, 0.0), Vec3::new(260.0, 500.0, 600.0));
    let mut prizes = Prizes::new(1);
    prizes.spawn(Vec3::new(0.0, 0.0, 150.0), PrizeClass::Orb(PrizeKind::Munny10), Vec3::new(4000.0, 0.0, 300.0), &SpawnMods::default(), false, 0);
    let mut bounced = false;
    for _ in 0..30 {
        prizes.step(DT, &world, &far());
        let p = prizes.list()[0];
        assert!(p.position.x <= 200.0 - 34.0 + 0.01, "inside the wall: {}", p.position.x);
        bounced |= p.velocity.x < 0.0;
    }
    assert!(bounced);
}

/// Spec 5: 50 frames before it can be collected, 15 s of life after that, then it is gone at once.
#[test]
fn an_orb_nobody_collects_vanishes_after_its_living_time() {
    let world = TestWorld::floor();
    let mut prizes = Prizes::new(1);
    prizes.spawn(Vec3::new(0.0, 0.0, 30.0), PrizeClass::Orb(PrizeKind::MpSmall), Vec3::ZERO, &SpawnMods::default(), false, 0);
    let mut expired_at = None;
    let mut collectible_at = None;
    for step in 1..=(17 * 60) {
        prizes.step(DT, &world, &Getter::NONE);
        if collectible_at.is_none() && prizes.list().first().is_some_and(|p| p.collectible()) {
            collectible_at = Some(step);
        }
        if prizes.take_events().iter().any(|e| matches!(e, PrizeEvent::Expired { .. })) {
            expired_at = Some(step);
            break;
        }
    }
    assert!((49..=51).contains(&collectible_at.unwrap()), "{collectible_at:?}");
    let seconds = expired_at.unwrap() as f32 * DT;
    assert!((seconds - (P.life_appear_time + P.life_living_time)).abs() < 0.05, "{seconds}");
    assert!(prizes.is_empty());
}

/// Spec 6: inside the magnet radius (300 cm; 600 / 900 with Treasure Magnets; any distance with
/// the master ability or a magic-draw drop) the orb flies to the getter in 40 frames on a
/// shrinking spiral, plays its get sound 12 frames before the end, and is applied on arrival.
#[test]
fn an_orb_in_range_spirals_to_the_getter_and_is_collected() {
    let world = TestWorld::floor();
    let getter = Getter { point: Vec3::new(0.0, 0.0, 91.0), can_take: true, radius: Some(P.magnet_radius[0]) };
    // Out of range it lies; a dead getter takes nothing.
    for (at, g) in [(Vec3::new(301.0, 0.0, 91.0), getter), (Vec3::new(100.0, 0.0, 91.0), Getter { can_take: false, ..getter })] {
        let mut prizes = Prizes::new(1);
        prizes.spawn(at, PrizeClass::Orb(PrizeKind::HpSmall), Vec3::ZERO, &SpawnMods::default(), false, 0);
        for _ in 0..120 {
            prizes.step(DT, &world, &g);
        }
        assert!(prizes.list()[0].state == State::Appear);
    }
    // Wider with the abilities; unlimited with the master ability or the magic-draw flag.
    for (radius, magic_draw) in [(Some(P.magnet_radius[1]), false), (None, false), (Some(300.0), true)] {
        let mut prizes = Prizes::new(1);
        prizes.spawn(Vec3::new(550.0, 0.0, 13.0), PrizeClass::Orb(PrizeKind::HpSmall), Vec3::ZERO, &SpawnMods::default(), magic_draw, 0);
        for _ in 0..60 {
            prizes.step(DT, &world, &Getter { radius, ..getter });
        }
        assert!(prizes.list()[0].state == State::Get, "{radius:?} {magic_draw}");
    }
    // The flight.
    let mut prizes = Prizes::new(1);
    prizes.spawn(Vec3::new(200.0, 0.0, 91.0), PrizeClass::Orb(PrizeKind::Munny5), Vec3::ZERO, &SpawnMods::default(), false, 0);
    prizes.take_events();
    let (mut started, mut sound, mut collected) = (None, None, None);
    let mut distances = Vec::new();
    let mut turned = 0.0f32;
    let mut last_angle = None;
    for step in 1..=200 {
        prizes.step(DT, &world, &getter);
        for e in prizes.take_events() {
            match e {
                PrizeEvent::GetStarted { .. } => started = Some(step),
                PrizeEvent::GetSound { .. } => sound = Some(step),
                PrizeEvent::Collected { class, amount, .. } => {
                    assert_eq!((class, amount), (PrizeClass::Orb(PrizeKind::Munny5), 0));
                    collected = Some(step);
                }
                _ => {}
            }
        }
        if let (Some(_), Some(p)) = (started, prizes.list().first()) {
            if p.state == State::Get {
                let offset = p.position - getter.point;
                distances.push(offset.length());
                let angle = offset.y.atan2(offset.x).to_degrees();
                if let Some(last) = last_angle {
                    turned += kh_core::sora::wrap_degrees(angle - last);
                }
                last_angle = Some(angle);
                assert!(p.scale <= 1.0 && p.scale >= 0.0);
            }
        }
        if collected.is_some() {
            break;
        }
    }
    let (started, sound, collected) = (started.unwrap(), sound.unwrap(), collected.unwrap());
    assert!((49..=51).contains(&started), "the appear time first: {started}");
    assert_eq!(collected - started, 40, "row get.time");
    assert!((11..=13).contains(&(collected - sound)), "row get.sound_at: {}", collected - sound);
    assert!(distances.windows(2).all(|w| w[1] < w[0] + 1.0e-3) && distances[0] <= 211.0 && *distances.last().unwrap() < 10.0, "it closes in: {distances:?}");
    assert!(turned > 90.0, "it circles him: {turned} degrees");
    assert!(prizes.is_empty());
}

/// Spec 6: the getter is followed while the orb flies.
#[test]
fn the_flight_follows_a_moving_getter() {
    let world = TestWorld::floor();
    let mut prizes = Prizes::new(1);
    prizes.spawn(Vec3::new(100.0, 0.0, 91.0), PrizeClass::Orb(PrizeKind::HpSmall), Vec3::ZERO, &SpawnMods::default(), false, 0);
    let mut point = Vec3::new(0.0, 0.0, 91.0);
    let mut arrived = None;
    for _ in 0..200 {
        point.x -= 980.0 * DT;
        prizes.step(DT, &world, &Getter { point, can_take: true, radius: None });
        if let Some(PrizeEvent::Collected { position, .. }) = prizes.take_events().into_iter().find(|e| matches!(e, PrizeEvent::Collected { .. })) {
            arrived = Some(position);
            break;
        }
    }
    assert_eq!(arrived, Some(point));
}

/// Spec 2: the abilities of a drop. PRIZE_FEEVER: count + (count + 1) / 2; the converters in the
/// order of the spawn function.
#[test]
fn abilities_change_the_count_and_the_kind_at_spawn() {
    let table = DropTable { prizes: [(1, 3), (7, 2)], ..DropTable::default() };
    let mut prizes = Prizes::new(3);
    prizes.drop_table(Vec3::ZERO, &table, &SpawnMods { prize_fever: true, ..SpawnMods::default() }, false);
    assert_eq!(prizes.list().len(), (3 + 2) + (2 + 1));
    use PrizeKind::*;
    let c = |hp, mp, focus, millionaire, munny| Converters { hp, mp, focus, millionaire, munny };
    assert_eq!(c(true, false, false, false, false).convert(HpSmall), HpLarge);
    assert_eq!(c(false, true, false, false, false).convert(MpSmall), MpLarge);
    assert_eq!(c(false, false, true, false, false).convert(FocusSmall), FocusLarge);
    assert_eq!(c(false, false, false, false, true).convert(Munny1), Munny5);
    assert_eq!(c(false, false, false, false, true).convert(Munny5), Munny10);
    // Rags to Riches: 0, 2, 7 -> munny 1; 1, 3, 4, 8 -> munny 5; 5, 6 -> munny 10.
    let rich = c(false, false, false, true, false);
    assert_eq!([HpSmall, MpSmall, FocusSmall].map(|k| rich.convert(k)), [Munny1; 3]);
    assert_eq!([HpLarge, MpLarge, Munny1, FocusLarge].map(|k| rich.convert(k)), [Munny5; 4]);
    assert_eq!([Munny5, Munny10].map(|k| rich.convert(k)), [Munny10; 2]);
    // The order: converter first, then Rags to Riches, then the munny converter.
    assert_eq!(c(true, false, false, true, false).convert(HpSmall), Munny5);
    assert_eq!(c(true, false, false, true, true).convert(HpSmall), Munny10);
    // A converted orb IS the other class (its effect, its radius).
    let mut prizes = Prizes::new(3);
    prizes.drop_orbs(Vec3::ZERO, HpSmall, 1, &[], &SpawnMods { converters: c(true, false, false, false, false), ..SpawnMods::default() }, false);
    assert_eq!(prizes.list()[0].class, PrizeClass::Orb(HpLarge));
}

/// Spec 2: item slots are rolled per slot against their percent rate (1 .. 99 <= rate); LUCK_UP
/// multiplies the rate; an item comes as a box that can be collected at once and lives 180 s.
#[test]
fn item_slots_roll_against_their_rate_and_drop_boxes() {
    let table = DropTable { items: [(10, 8), (23, 4), (0, 0)], ..DropTable::default() };
    let (mut potions, mut materials) = (0, 0);
    let mut prizes = Prizes::new(11);
    for _ in 0..4000 {
        prizes.drop_table(Vec3::ZERO, &table, &SpawnMods::default(), false);
    }
    for p in prizes.list() {
        match p.class {
            PrizeClass::Box(BoxKind::Heal, 0x101) => potions += 1,
            PrizeClass::Box(BoxKind::Material, 0x701) => materials += 1,
            other => panic!("{other:?}"),
        }
        assert!(p.collectible() && p.time_left() == Some(180.0));
        assert!(p.velocity.z >= 600.0 && p.velocity.z < 1200.0 && p.velocity.x.abs() < 300.0);
    }
    // 8 / 99 and 4 / 99 of 4000.
    assert!((250..400).contains(&potions) && (110..220).contains(&materials), "{potions} {materials}");
    let mut lucky = Prizes::new(11);
    for _ in 0..4000 {
        lucky.drop_table(Vec3::ZERO, &table, &SpawnMods { luck_up: 2, ..SpawnMods::default() }, false);
    }
    // x 1.6: rates 13 and 6.
    assert!(lucky.list().len() as f32 > prizes.list().len() as f32 * 1.35);
    // One roll for the three: never more than one item.
    let one = DropTable { items: [(10, 50), (11, 50), (12, 50)], one_item: true, ..DropTable::default() };
    let mut prizes = Prizes::new(5);
    for _ in 0..200 {
        assert!(prizes.drop_table(Vec3::ZERO, &one, &SpawnMods::default(), false) <= 1);
    }
    assert!(prizes.list().len() > 150);
}

/// Spec 7 / stats spec 6.3: what each orb gives, through the status path of the core; the get and
/// bounce sounds are seam sounds named "<class>.get" / "<class>.bound"; munny raises the munny
/// popup of the HUD; a host amount replaces the value of a munny orb.
#[test]
fn collected_orbs_are_applied_through_the_status_path_of_the_core() {
    let world = TestWorld::floor();
    let mut sora = sora99();
    let (max_hp, max_mp) = (sora.status().max_hp(), sora.status().max_mp());
    sora.status_mut().hp = 100;
    sora.status_mut().mp = 50;
    sora.status_mut().focus = 10;
    sora.step(DT, &Input::default(), &world);
    assert!(max_hp > 110 && max_mp > 60);
    let mut prizes = Prizes::new(2);
    let mods = host::mods_of(&sora);
    assert_eq!(mods, SpawnMods::default(), "the level-99 loadout wears no prize converter");
    let centre = sora.output().capsule_center;
    for (k, kind) in PrizeKind::ALL.into_iter().enumerate() {
        prizes.spawn(centre + Vec3::new(150.0, 20.0 * k as f32, 0.0), PrizeClass::Orb(kind), Vec3::ZERO, &mods, false, if kind == PrizeKind::Munny10 { 730 } else { 0 });
    }
    let mut sounds = Vec::new();
    let mut munny_popup = 0.0f32;
    for _ in 0..200 {
        let getter = Getter::sora(&sora, sora.output().capsule_center);
        prizes.step(DT, &world, &getter);
        for event in prizes.take_events() {
            host::apply_to_sora(&mut sora, &event);
        }
        let out = sora.step(DT, &Input::default(), &world);
        for e in &out.events {
            match e.kind {
                kh_core::EventKind::SoundPlay { play, .. } => sounds.push(play.name),
                kh_core::EventKind::Popup(p) if p.kind == kh_core::UiPopup::KIND_MUNNY => munny_popup += p.value,
                _ => {}
            }
        }
    }
    assert!(prizes.is_empty());
    let s = sora.status();
    assert_eq!((s.hp, s.mp, s.focus), (100 + 1 + 5, 50 + 3 + 5, 10 + 3 + 5), "row kind.value");
    assert_eq!((s.munny, munny_popup), (1 + 5 + 730, 736.0));
    for name in ["PrizeHpS.get", "PrizeHpL.get", "PrizeMpS.get", "PrizeMpL.get", "PrizeMunnyS.get", "PrizeMunnyM.get", "PrizeMunnyL.get", "PrizeFocusS.get", "PrizeFocusL.get"] {
        assert_eq!(sounds.iter().filter(|s| **s == name).count(), 1, "{name}: {sounds:?}");
    }
    // The effect of each class is one of the prize effects of the pack at the scale of its blueprint.
    assert_eq!(host::effect(PrizeClass::Orb(PrizeKind::HpSmall)), ("vs_plz_hpb0", 0.5));
    assert_eq!(host::effect(PrizeClass::Orb(PrizeKind::Munny5)), ("VS_PLZ_MNY0", 0.75));
    assert_eq!(host::sound_name(PrizeClass::Box(BoxKind::Heal, 0x101), true), "PrizeBoxHeal.bound");
}
