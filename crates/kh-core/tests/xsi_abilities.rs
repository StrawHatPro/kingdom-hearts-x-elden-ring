//! (XSI, 2026-10-16) THE XS ABILITIES (build/sora_tmp/party/XS_SORA_ABILITIES.md; crates/kh-core/src/abilities/xs/):
//! one test per ability, through the pad path (`Input::pad`, the stick), against training dummies of the recording
//! host. Every number the tests compare with is a row of sheets/port/sora-xs-abilities (`xs::X`) or of the KH3
//! attack rows.

use kh_core::abilities::xs::{self, X};
use kh_core::enums::{ETresAbilityKind as A, ETresDamageKind};
use kh_core::host::{BodyReaction, EventKind, HitRecord, TargetBody};
use kh_core::magic::View;
use kh_core::states::HitOnPlayer;
use kh_core::status::LoadoutValue;
use kh_core::test_world::TestWorld;
use kh_core::{AnimId, Input, Loadout, Output, PadButton, PadButtons, PadState, Pose, RecordingHost, Sora, StateId, Status, TargetFlags, TargetId, TargetInfo, Tuning, Vec2, Vec3, SORA_PARAMS};

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

/// The level-99 loadout sheet as it is (its default equipped set), plus the XS abilities `extra`, owned and
/// equipped whatever the AP (a test configuration).
fn level99(extra: &[A]) -> Status {
    let path = format!("{}/../../sheets/port/sora-loadout-99.json", env!("CARGO_MANIFEST_DIR"));
    let json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let mut loadout = Loadout::new("sora-loadout-99");
    for row in json["rows"].as_array().unwrap() {
        loadout.set(row[0].as_str().unwrap(), &cell(&row[1]).unwrap()).unwrap();
    }
    loadout.resolve().unwrap();
    let mut status = Status::from_loadout(&loadout);
    for &kind in extra {
        let owned = status.abilities.owned(kind).max(1);
        status.abilities.set(kind, owned, 1);
    }
    status
}

fn dummy(id: u32, at: Vec3) -> TargetInfo {
    TargetInfo { id: TargetId(id), position: at, center: at + Vec3::Z * 90.0, velocity: Vec3::ZERO, yaw: 180.0, radius: 40.0, half_height: 90.0, flags: TargetFlags(TargetFlags::ALIVE | TargetFlags::HITTABLE | TargetFlags::LOCKABLE | TargetFlags::ENEMY | if at.z > 1.0 { TargetFlags::AIRBORNE } else { 0 }), hp: 1.0e6, hp_max: 1.0e6 }
}

struct Bench {
    sora: Sora,
    host: RecordingHost,
    world: TestWorld,
    held: PadButtons,
    stick: Vec2,
    view: Option<View>,
    outs: Vec<Output>,
}

impl Bench {
    fn at(status: Status, dummies: &[TargetInfo], position: Vec3) -> Bench {
        let origin = position + Vec3::Z * Sora::origin_height_on_floor(&SORA_PARAMS);
        let sora = Sora::new(SORA_PARAMS, Pose { position: origin, yaw: 0.0 }).with_tuning(Tuning::GAME_EXACT).with_status(status);
        let mut host = RecordingHost::new();
        host.targets.extend_from_slice(dummies);
        Bench { sora, host, world: TestWorld::floor(), held: PadButtons::NONE, stick: Vec2::ZERO, view: Some(View::from_camera(Vec3::new(-300.0, 0.0, 150.0), 0.0, 0.0, 90.0, 1280, 720)), outs: Vec::new() }
    }

    fn new(status: Status, dummies: &[TargetInfo]) -> Bench {
        let mut b = Bench::at(status, dummies, Vec3::ZERO);
        b.run(5);
        b
    }

    fn step(&mut self, held: &[PadButton]) -> Output {
        let now = held.iter().fold(PadButtons::NONE, |all, b| all.with(*b));
        let pad = PadState::from_held(now, self.held);
        self.held = now;
        let mut input = Input::default().with_pad(pad);
        input.view = self.view;
        input.move_stick = self.stick;
        let (sora, world) = (&mut self.sora, &self.world);
        let out = self.host.step(world, |h| sora.step_hosted(DT, &input, h));
        self.outs.push(out.clone());
        out
    }

    fn run(&mut self, steps: usize) -> Output {
        let mut out = self.outs.last().cloned();
        for _ in 0..steps {
            out = Some(self.step(&[]));
        }
        out.unwrap()
    }

    fn until(&mut self, limit: usize, held: &[PadButton], done: impl Fn(&Output, &Sora) -> bool) -> usize {
        for k in 1..=limit {
            let out = self.step(held);
            if done(&out, &self.sora) {
                return k;
            }
        }
        let o = self.outs.last().unwrap();
        panic!("not reached in {limit} steps: state {:?} sub {} anim {:?} at {:?}", o.state, o.sub_step, o.anim.name, o.position);
    }

    fn hits(&self) -> Vec<HitRecord> {
        self.host.hits().into_iter().map(|h| h.0).collect()
    }

    fn hits_of(&self, row: &str) -> Vec<HitRecord> {
        self.hits().into_iter().filter(|h| h.attack_name == row).collect()
    }

    fn lock(&mut self, id: u32) {
        let lock = &mut self.sora.combat_mut().lockon;
        lock.locked = true;
        lock.current = TargetId(id);
    }
}

// ------------------------------------------------------------------------------------ 2.1 Diving Strike

/// The END hit of a dive from `height` (cm above the floor) onto a dummy below: (its damage, the dive's kinds).
fn dive_from(height: f32) -> (f32, Vec<u8>, bool) {
    let mut b = Bench::at(level99(&[A::DivingStrike]), &[dummy(1, Vec3::new(300.0, 0.0, 0.0))], Vec3::new(0.0, 0.0, height));
    // He falls (Jump, fall entry); 0.2 s later the press.
    b.until(60, &[], |o, _| o.state == StateId::Jump && o.velocity.z < 0.0);
    b.run(10);
    b.step(&[PadButton::ATTACK]);
    let mut invincible = false;
    for _ in 0..400 {
        let out = b.step(&[]);
        invincible |= out.state == StateId::Attack && b.sora.status().damage_invalid && b.sora.combat().xs.dive.is_some_and(|d| d.holding);
        if out.state != StateId::Attack && !b.hits_of("DIVEFALL_ATTACK_END").is_empty() {
            break;
        }
    }
    let end = b.hits_of("DIVEFALL_ATTACK_END");
    assert!(!end.is_empty(), "the END hit landed (kinds {:?})", b.sora.combat().history);
    (end[0].damage, b.sora.combat().history.clone(), invincible)
}

#[test]
fn xsi_diving_strike_homes_onto_a_target_below_and_the_drop_adds_power() {
    let (high, kinds, invincible) = dive_from(3600.0);
    assert_eq!(kinds.last(), Some(&xs::kind::DIVE), "the press out of the fall forced KH3's dive-fall attack 0x35");
    assert!(invincible, "invincible while the dive pose homes");
    let (low, _, _) = dive_from(X.dive_min_drop + 150.0);
    // A drop of about 33 m: +10 % per full 5 m beyond 6 m, capped at +50 %; the low dive gets nothing.
    let ratio = high / low;
    assert!((ratio - (1.0 + X.dive_bonus_max)).abs() < 0.05, "high {high} / low {low} = {ratio}");
    assert_eq!(xs::dive::fall_bonus(X.dive_min_drop + 499.0), 0.0);
    assert!((xs::dive::fall_bonus(X.dive_min_drop + 1000.0) - 2.0 * X.dive_bonus_per_step).abs() < 1e-6);
    // Without a target in KH3's cone (1000 cm horizontally) the press is the normal air attack.
    let mut b = Bench::at(level99(&[A::DivingStrike]), &[dummy(1, Vec3::new(1500.0, 0.0, 0.0))], Vec3::new(0.0, 0.0, 3000.0));
    b.until(60, &[], |o, _| o.state == StateId::Jump && o.velocity.z < 0.0);
    b.step(&[PadButton::ATTACK]);
    b.run(3);
    assert_ne!(b.sora.combat().history.last(), Some(&xs::kind::DIVE));
    // Too low (less than dive.min_drop below): no dive either.
    let mut b = Bench::at(level99(&[A::DivingStrike]), &[dummy(1, Vec3::new(200.0, 0.0, 0.0))], Vec3::new(0.0, 0.0, X.dive_min_drop - 200.0));
    b.until(60, &[], |o, _| o.state == StateId::Jump && o.velocity.z < 0.0);
    b.step(&[PadButton::ATTACK]);
    b.run(3);
    assert_ne!(b.sora.combat().history.last(), Some(&xs::kind::DIVE));
}

#[test]
fn xsi_diving_strike_head_first_pose_after_a_second_of_free_fall() {
    // From 60 m with nothing below: after dive.pose_fall_time of falling the head-first pose (DiveFallStart, then
    // the DiveFallLoop blend space); RollLand on the floor. Without the ability, the down loop all the way.
    for (with, expect) in [(true, true), (false, false)] {
        let mut b = Bench::at(level99(if with { &[A::DivingStrike] } else { &[] }), &[], Vec3::new(0.0, 0.0, 6000.0));
        let (mut start, mut looped, mut rolled) = (false, false, false);
        for _ in 0..600 {
            let out = b.step(&[]);
            start |= out.anim.name == AnimId::DiveFallStart;
            looped |= out.anim.name == AnimId::DiveFallLoop;
            rolled |= start && out.anim.name == AnimId::RollLand;
            if out.state == StateId::Idle && out.on_floor && b.outs.len() > 30 {
                break;
            }
        }
        assert_eq!((start, looped, rolled), (expect, expect, expect), "with the ability: {with}");
    }
}

// ------------------------------------------------------------------------------------ 2.2 Sonic Dash

#[test]
fn xsi_sonic_dash_flashes_through_a_locked_target_and_the_second_ends_in_the_launch() {
    let foe = dummy(1, Vec3::new(500.0, 0.0, 700.0));
    let mut b = Bench::at(level99(&[A::SonicDash]), &[foe], Vec3::new(0.0, 0.0, 700.0));
    b.sora.force_battle(true);
    b.until(60, &[], |o, _| o.state == StateId::Jump);
    b.lock(1);
    // X with the stick toward the target (+X: camera yaw 0, stick forward).
    b.stick = Vec2::new(0.0, 1.0);
    b.step(&[PadButton::GUARD]);
    b.stick = Vec2::ZERO;
    let out = b.step(&[]);
    assert_eq!((out.state, b.sora.combat().history.last()), (StateId::Attack, Some(&xs::kind::SONIC_DASH)), "the dash");
    assert!(b.sora.status().damage_invalid, "invincible while it dashes");
    let x0 = out.position.x;
    b.run((X.sonic_dash_time / DT) as usize + 2);
    let passed = b.outs.last().unwrap().position.x - x0;
    assert!(passed > 0.75 * X.sonic_dash_length, "the dash moved {passed} cm");
    assert!(!b.sora.status().damage_invalid, "the invincibility ends with the dash part");
    assert!(!b.hits_of("LM_SONICRAVE").is_empty(), "LM_SONICRAVE hit the target on the way");
    // The second dash back through it, then Attack within the follow window: M_1426.
    let back = b.outs.last().unwrap().position.x > 500.0;
    b.until(60, &[], |_, s| s.attack().is_none_or(|a| a.enable_input));
    b.stick = Vec2::new(0.0, if back { -1.0 } else { 1.0 });
    b.step(&[PadButton::GUARD]);
    b.stick = Vec2::ZERO;
    b.run(2);
    assert_eq!(b.sora.combat().xs.sonic.total, 2, "two dashes");
    b.until(90, &[], |_, s| s.combat().xs.sonic.running.is_none());
    b.step(&[PadButton::ATTACK]);
    b.until(60, &[], |_, s| s.combat().history.contains(&xs::kind::SONIC_LAUNCH));
    assert!(b.sora.combat().history.contains(&xs::kind::SONIC_LAUNCH), "the launch after the 2nd dash: {:?}", b.sora.combat().history);
    b.run(80);
    assert!(!b.hits_of("LM_SONICRAVE_END").is_empty(), "the TOSS launch hit");
    // Without a lock, X is the Air Slide.
    let mut b = Bench::at(level99(&[A::SonicDash]), &[foe], Vec3::new(0.0, 0.0, 700.0));
    b.sora.force_battle(true);
    b.until(60, &[], |o, _| o.state == StateId::Jump);
    b.stick = Vec2::new(0.0, 1.0);
    b.step(&[PadButton::GUARD]);
    assert_eq!(b.step(&[]).state, StateId::AirSlide);
}

// ------------------------------------------------------------------------------------ 2.13 Ancient Light

#[test]
fn xsi_ancient_light_spends_the_gauge_against_a_powerful_foe_within_the_25_percent_cap() {
    let mut foe = dummy(1, Vec3::new(250.0, 0.0, 0.0));
    (foe.hp, foe.hp_max) = (2000.0, 2000.0);
    let mut b = Bench::new(level99(&[A::AncientLight]), &[foe]);
    b.host.bodies.push((TargetId(1), TargetBody { reaction: BodyReaction::Strong, ..TargetBody::DEFAULT }));
    b.host.endurance.push((TargetId(1), 1000));
    b.lock(1);
    b.sora.debug_add_style(100.0);
    b.run(30);
    let q = &b.sora.combat().situation;
    assert!(q.has(0x94) && q.has(xs::ancient::COMMAND), "Second Form and Ancient Light offered");
    b.step(&[PadButton::SITUATION]);
    b.run(1);
    assert_eq!(b.sora.combat().history.last(), Some(&xs::kind::ANCIENT_LIGHT));
    assert!(!b.sora.combat().situation.has(0x94), "the gauge's Second Form offer was spent");
    assert!(b.sora.status().damage_invalid);
    b.until(400, &[], |o, _| o.state != StateId::Attack);
    let dealt: f32 = b.hits().iter().filter(|h| h.target == TargetId(1) && h.attack_name.starts_with("LM_UNIONDISCODE")).map(|h| h.damage).sum();
    assert!(dealt > 0.0 && dealt <= X.spectacle_cap_share * 2000.0 + 1e-3, "dealt {dealt} of the cap {}", X.spectacle_cap_share * 2000.0);
    assert!(b.hits().iter().any(|h| h.attack_name == "LM_UNIONDISCODE4"), "the 6.0 BLOW landed");
    assert!(!b.sora.status().damage_invalid);
    // A target that is not a powerful foe: Second Form only.
    let mut b = Bench::new(level99(&[A::AncientLight]), &[dummy(1, Vec3::new(250.0, 0.0, 0.0))]);
    b.lock(1);
    b.sora.debug_add_style(100.0);
    b.run(30);
    assert!(b.sora.combat().situation.has(0x94) && !b.sora.combat().situation.has(xs::ancient::COMMAND));
}

// ------------------------------------------------------------------------------------ 2.8 Spectral Rays

#[test]
fn xsi_spectral_rays_is_the_shotlock_when_chosen_and_its_beams_refract() {
    let foes = [dummy(1, Vec3::new(800.0, 0.0, 0.0)), dummy(2, Vec3::new(800.0, 150.0, 0.0))];
    let mut b = Bench::new(level99(&[A::ShotlockSpectralRays]), &foes);
    b.sora.command_mut().shotlock = xs::spectral::KIND;
    for _ in 0..80 {
        b.step(&[PadButton::RightShoulder]);
    }
    let aim = b.sora.aim();
    assert_eq!((aim.mode, aim.kind, aim.max_locks), (1, xs::spectral::KIND, 28), "row 11 of TresShootFlowData");
    let total = aim.total;
    assert!(total >= 2);
    b.step(&[PadButton::RightShoulder, PadButton::ATTACK]);
    let out = b.step(&[PadButton::RightShoulder]);
    assert_eq!(out.state, StateId::Shootflow);
    b.until(400, &[], |o, _| o.state != StateId::Shootflow);
    let direct = b.hits_of("SF_REFLECTIVELASER");
    assert!(direct.len() as i32 >= total, "{} beam hits for {total} locks", direct.len());
    // Each beam refracts to the other dummy at half the damage (the same stats on both).
    let full = direct.iter().map(|h| h.damage).fold(0.0, f32::max);
    let half = direct.iter().map(|h| h.damage).fold(f32::INFINITY, f32::min);
    assert!((half - full * X.spectral_refract_scale).abs() <= 1.0, "full {full}, refracted {half}");
    assert!(b.sora.combat().history.is_empty() || !b.sora.combat().history.contains(&0));
    // Not chosen (or chosen without the ability): the Keyblade's own King of Hearts.
    let mut b = Bench::new(level99(&[]), &foes);
    b.sora.command_mut().shotlock = xs::spectral::KIND;
    for _ in 0..40 {
        b.step(&[PadButton::RightShoulder]);
    }
    assert_eq!(b.sora.aim().kind, 1);
}

// ------------------------------------------------------------------------------------ 2.10 Critical Counter

#[test]
fn xsi_critical_counter_just_guard_wears_a_strong_bodys_endurance() {
    let guard_then_hit = |equipped: bool, strong: bool, wait: usize| {
        let foe = dummy(1, Vec3::new(150.0, 0.0, 0.0));
        let mut b = Bench::new(level99(if equipped { &[A::CriticalCounter] } else { &[] }), &[foe]);
        if strong {
            b.host.bodies.push((TargetId(1), TargetBody { reaction: BodyReaction::Strong, ..TargetBody::DEFAULT }));
        }
        b.host.endurance.push((TargetId(1), 25));
        b.step(&[PadButton::GUARD]);
        b.run(wait);
        let own = b.outs.last().unwrap().position;
        b.sora.take_hit(HitOnPlayer { direction: Vec3::new(-1.0, 0.0, 0.0), ..HitOnPlayer::new(ETresDamageKind::Small, 30.0, TargetId(1), foe.position + (own - foe.position) * 0.0) });
        b.run(1);
        (b.sora.actions().critical, b.host.endurance[0].1)
    };
    let rp = X.critical_counter_strong_rp as i32;
    assert_eq!(guard_then_hit(true, true, 4), (true, 25 - rp), "a just guard on a STRONG body");
    assert_eq!(guard_then_hit(true, true, 12), (false, 25), "too late: no just guard");
    assert_eq!(guard_then_hit(true, false, 4), (true, 25), "an ordinary body keeps its endurance");
    assert_eq!(guard_then_hit(false, true, 4), (false, 25), "without the ability");
    // The catalogue: level C 18 and Godskin Noble (not the Critical start: DF, dec032), AP 5.
    let row = kh_core::status::tables::ability_row(A::CriticalCounter).unwrap();
    assert_eq!((row.ap, row.owner), (Some(5), "sora"));
    assert_eq!((kh_core::status::tables::level_ability(18, 2), kh_core::status::tables::level_ability(18, 0)), (Some(A::CriticalCounter), Some(A::WaterUp)));
    let _ = EventKind::AimMode { open: false, reason: 0 };
}

// ------------------------------------------------------------------------------------ the finishers (2.3 - 2.6, 2.12)

impl Bench {
    /// Attack taps (`hold` steps down, one up) until `done` or `limit` steps; `keep` runs before each step.
    fn combo(&mut self, limit: usize, hold: usize, keep: impl Fn(&mut Bench), done: impl Fn(&Bench) -> bool) {
        for i in 0..limit {
            keep(self);
            if i % (hold + 1) < hold {
                self.step(&[PadButton::ATTACK]);
            } else {
                self.step(&[]);
            }
            if done(self) {
                return;
            }
        }
    }

    fn kinds(&self) -> Vec<u8> {
        self.sora.combat().history.clone()
    }

    fn names(&self) -> Vec<&'static str> {
        self.hits().iter().map(|h| h.attack_name).collect()
    }
}

fn strong(b: &mut Bench, id: u32) {
    b.host.bodies.push((TargetId(id), TargetBody { reaction: BodyReaction::Strong, ..TargetBody::DEFAULT }));
    b.host.endurance.push((TargetId(id), 1000));
}

#[test]
fn xsi_spiral_rush_is_the_finisher_against_a_target_5_to_12_m_away() {
    let mut b = Bench::new(level99(&[A::SpiralRush]), &[dummy(1, Vec3::new(200.0, 0.0, 0.0))]);
    b.lock(1);
    // After two hits the dummy keeps 8 m ahead (the choice is made when the finisher begins).
    b.combo(
        600,
        1,
        |b| {
            if b.sora.combat().history.len() >= 2 && !b.kinds().contains(&xs::finishers::SPIRAL_RUSH) {
                let p = b.outs.last().map_or(Vec3::ZERO, |o| o.position);
                b.host.targets[0].position = Vec3::new(p.x + 800.0, p.y, 0.0);
                b.host.targets[0].center = Vec3::new(p.x + 800.0, p.y, 90.0);
            }
        },
        |b| b.kinds().contains(&xs::finishers::SPIRAL_RUSH),
    );
    assert!(b.kinds().contains(&xs::finishers::SPIRAL_RUSH), "{:?}", b.kinds());
    b.run(150);
    assert!(b.names().iter().any(|n| n.starts_with("SCLF_SPIRALRUSH")), "the drill reached it: {:?}", b.names());
    // Near: the usual finisher.
    let mut b = Bench::new(level99(&[A::SpiralRush]), &[dummy(1, Vec3::new(200.0, 0.0, 0.0))]);
    b.combo(600, 1, |_| {}, |b| b.sora.combat().history.len() >= 7);
    assert!(!b.kinds().contains(&xs::finishers::SPIRAL_RUSH), "{:?}", b.kinds());
}

/// The combo with Attack held `hold` steps per press, the last press held `last` steps: (kinds, charge, damage of
/// the first SCLF_ZANTETSUKEN hit).
fn zantetsuken(last: usize) -> (Vec<u8>, f32, f32) {
    let mut b = Bench::new(level99(&[A::Zantetsuken]), &[dummy(1, Vec3::new(200.0, 0.0, 0.0))]);
    let hold = (X.zantetsuken_hold_min / DT) as usize + 12;
    b.combo(900, hold, |_| {}, |b| b.kinds().contains(&xs::finishers::ZANTETSUKEN));
    let mut charge: f32 = 0.0;
    for _ in 0..last {
        b.step(&[PadButton::ATTACK]);
        charge = charge.max(b.sora.combat().xs.zanto.map_or(0.0, |z| z.charge));
    }
    b.run(150);
    let hit = b.hits().iter().find(|h| h.attack_name.starts_with("SCLF_ZANTETSUKEN")).map_or(0.0, |h| h.damage);
    (b.kinds(), charge, hit)
}

#[test]
fn xsi_zantetsuken_is_the_held_finisher_and_a_full_charge_cuts_harder() {
    let (kinds, charge, full) = zantetsuken(120);
    assert!(kinds.contains(&xs::finishers::ZANTETSUKEN), "{kinds:?}");
    assert!(charge >= X.zantetsuken_full_charge, "the stance was held {charge} s");
    let (kinds, charge, short) = zantetsuken(((X.zantetsuken_stance_time + 0.2) / DT) as usize);
    assert!(kinds.contains(&xs::finishers::ZANTETSUKEN) && charge < X.zantetsuken_full_charge, "{kinds:?} {charge}");
    assert!(full > 0.0 && short > 0.0, "full {full} short {short}");
    assert!((full / short - X.zantetsuken_full_power).abs() < 0.08, "full {full} short {short}");
    // Taps only: never Zantetsuken.
    let mut b = Bench::new(level99(&[A::Zantetsuken]), &[dummy(1, Vec3::new(200.0, 0.0, 0.0))]);
    b.combo(600, 1, |_| {}, |b| b.sora.combat().history.len() >= 7);
    assert!(!b.kinds().contains(&xs::finishers::ZANTETSUKEN), "{:?}", b.kinds());
}

#[test]
fn xsi_event_horizon_takes_magic_flashs_place() {
    for (with, want) in [(false, xs::finishers::MAGIC_FLASH), (true, xs::finishers::EVENT_HORIZON)] {
        let mut b = Bench::new(level99(if with { &[A::EventHorizon] } else { &[] }), &[dummy(1, Vec3::new(200.0, 0.0, 0.0)), dummy(2, Vec3::new(200.0, 150.0, 0.0)), dummy(3, Vec3::new(200.0, -150.0, 0.0)), dummy(4, Vec3::new(-150.0, 0.0, 0.0))]);
        b.combo(1500, 1, |_| {}, |b| b.kinds().contains(&want));
        assert!(b.kinds().contains(&want), "with {with}: {:?}", b.kinds());
        if with {
            assert!(!b.kinds().contains(&xs::finishers::MAGIC_FLASH));
            b.run(150);
            assert!(b.names().iter().any(|n| n.starts_with("SCLF_EVENTHORIZON")), "the ring hit: {:?}", b.names());
        }
    }
}

#[test]
fn xsi_giant_stamp_breaks_a_strong_body_and_is_invincible_at_the_top() {
    let mut b = Bench::new(level99(&[A::GiantStamp]), &[dummy(1, Vec3::new(200.0, 0.0, 0.0))]);
    strong(&mut b, 1);
    b.lock(1);
    b.combo(600, 1, |_| {}, |b| b.kinds().contains(&xs::finishers::GIANT_STAMP));
    assert!(b.kinds().contains(&xs::finishers::GIANT_STAMP), "{:?}", b.kinds());
    let (mut top, mut before) = (false, false);
    for _ in 0..200 {
        b.step(&[]);
        let t = b.outs.last().unwrap().anim.time;
        if b.sora.attack().is_some_and(|a| a.kind == xs::finishers::GIANT_STAMP) {
            if t > X.giant_stamp_invincible_from + 0.05 && t < X.giant_stamp_invincible_to - 0.05 {
                top |= b.sora.status().damage_invalid;
            }
            if t < X.giant_stamp_invincible_from - 0.1 {
                before |= b.sora.status().damage_invalid;
            }
        }
    }
    assert!(top && !before, "invincible at the top only");
    assert!(b.names().iter().any(|n| n.starts_with("SCHM_GIANTSTAMP")), "{:?}", b.names());
    // A light body: KH3's finisher.
    let mut b = Bench::new(level99(&[A::GiantStamp]), &[dummy(1, Vec3::new(200.0, 0.0, 0.0))]);
    b.combo(600, 1, |_| {}, |b| b.sora.combat().history.len() >= 7);
    assert!(!b.kinds().contains(&xs::finishers::GIANT_STAMP));
}

// ------------------------------------------------------------------------------------ 2.7 Chandelier

#[test]
fn xsi_chandelier_is_the_blizzard_finish_end() {
    use kh_core::command::CommandRequest;
    use kh_core::magic::spells::cmd;
    let run = |spell: u8, with: bool| {
        let mut b = Bench::new(level99(if with { &[A::Chandelier] } else { &[] }), &[dummy(1, Vec3::new(600.0, 0.0, 0.0))]);
        b.lock(1);
        for i in 0..900 {
            b.sora.status_mut().set_mp(999);
            if i % 2 == 0 {
                b.sora.command_request(CommandRequest::Execute(spell));
            }
            b.step(&[]);
            if b.kinds().contains(&xs::chandelier::CHANDELIER) {
                break;
            }
        }
        b.run(240);
        b
    };
    let b = run(cmd::BLIZZARD, true);
    assert!(b.kinds().contains(&xs::chandelier::CHANDELIER), "{:?}", b.kinds());
    assert!(run(cmd::BLIZZAGA, true).kinds().contains(&xs::chandelier::CHANDELIER), "Blizzaga too");
    let end = b.hits_of("SCSK_CHANDELIER_END");
    assert!(!end.is_empty(), "the crash: {:?}", b.names());
    assert_eq!(end[0].attribute, kh_core::enums::ETresDamageAttribute::Blizzard);
    // Fire, or no ability: KH3's Finish End.
    assert!(!run(cmd::FIRE, true).kinds().contains(&xs::chandelier::CHANDELIER));
    assert!(!run(cmd::BLIZZARD, false).kinds().contains(&xs::chandelier::CHANDELIER));
    assert_eq!((xs::chandelier::blizzard_tier(33), xs::chandelier::blizzard_tier(35), xs::chandelier::blizzard_tier(29)), (1, 3, 0));
}

// ------------------------------------------------------------------------------------ 2.14 Myriad

#[test]
fn xsi_myriad_ends_second_form_with_the_ultima_finale_within_the_cap() {
    let mut foe = dummy(1, Vec3::new(250.0, 0.0, 0.0));
    (foe.hp, foe.hp_max) = (4000.0, 4000.0);
    let mut b = Bench::new(level99(&[A::Myriad]), &[foe]);
    b.lock(1);
    b.sora.debug_add_style(100.0);
    b.run(15);
    b.step(&[PadButton::SITUATION]);
    assert_eq!(b.step(&[]).state, StateId::Stylechange);
    b.run(220);
    assert!(b.sora.combat().form.finish_pending, "the finish command is offered");
    let before = b.hits().len();
    b.step(&[PadButton::SITUATION]);
    b.run(2);
    assert_eq!(b.sora.combat().history.last(), Some(&xs::myriad::MYRIAD), "{:?}", b.kinds());
    assert!(b.sora.status().damage_invalid);
    b.until(1200, &[], |_, s| s.combat().form.cmd == 0);
    let dealt: f32 = b.hits()[before..].iter().filter(|h| h.target == TargetId(1)).map(|h| h.damage).sum();
    assert!(dealt > 0.0 && dealt <= X.spectacle_cap_share * 4000.0 + 1e-3, "{dealt}");
    assert!(b.names().iter().any(|n| n.starts_with("SCSW_MYRIAD")), "{:?}", b.names());
    b.run(60);
    assert!(!b.sora.status().damage_invalid);
}

// ------------------------------------------------------------------------------------ 2.9 Light Wave

#[test]
fn xsi_light_wave_is_offered_once_per_mp_charge_and_its_waves_pierce() {
    let foes = [dummy(1, Vec3::new(600.0, 0.0, 0.0)), dummy(2, Vec3::new(1200.0, 0.0, 0.0))];
    let command = X.light_wave_command as i32;
    let mut b = Bench::new(level99(&[A::LightWave]), &foes);
    b.sora.status_mut().set_mp_charge(true, 0.0);
    b.run(30);
    assert!(b.sora.combat().situation.has(command), "offered while MP charges");
    let charge = b.sora.status().mp_charge_timer;
    b.step(&[PadButton::SITUATION]);
    b.run(60);
    assert_eq!(b.sora.combat().history.last(), Some(&xs::light_wave::TAP), "{:?}", b.kinds());
    assert!(b.sora.status().mp_charge_timer > charge + X.light_wave_charge_extra - 2.0, "the charge got longer");
    let hit: Vec<TargetId> = b.hits_of(xs::light_wave::ROW).iter().map(|h| h.target).collect();
    assert!(hit.contains(&TargetId(1)) && hit.contains(&TargetId(2)), "the wave pierced: {hit:?}");
    assert_eq!(b.sora.combat().xs.wave.total, 1);
    assert!(!b.sora.combat().situation.has(command), "once per charge");
    // Held: the volley.
    let mut b = Bench::new(level99(&[A::LightWave]), &foes);
    b.sora.status_mut().set_mp_charge(true, 0.0);
    b.run(30);
    for _ in 0..180 {
        b.step(&[PadButton::SITUATION]);
    }
    b.run(60);
    assert_eq!(b.sora.combat().xs.wave.total, X.light_wave_volley as u32, "the volley's waves");
    // Not charging: no offer.
    let mut b = Bench::new(level99(&[A::LightWave]), &foes);
    b.run(30);
    assert!(!b.sora.combat().situation.has(command));
}

// ------------------------------------------------------------------------------------ 2.11 Rage Raid, 2.12 Riskcharge

fn guarded_hit(b: &mut Bench, foe: &TargetInfo) {
    b.step(&[PadButton::GUARD]);
    b.run(4);
    b.sora.take_hit(HitOnPlayer { direction: Vec3::new(-1.0, 0.0, 0.0), ..HitOnPlayer::new(ETresDamageKind::Small, 30.0, foe.id, foe.position) });
    b.run(2);
}

#[test]
fn xsi_rage_raid_answers_a_revenge_and_ends_it() {
    let foe = dummy(1, Vec3::new(150.0, 0.0, 0.0));
    let mut b = Bench::new(level99(&[A::RageRaid]), &[foe]);
    b.host.revenge.push(TargetId(1));
    guarded_hit(&mut b, &foe);
    assert!(b.sora.combat().xs.rage.raid_until > 0.0, "Raid offered");
    b.step(&[PadButton::ATTACK]);
    b.until(60, &[], |_, s| s.combat().history.last() == Some(&xs::rage::RAGE_RAID));
    assert!(b.sora.status().damage_invalid, "invincible at its start");
    b.run(200);
    assert!(!b.hits_of("AN_COUNTER_END").is_empty(), "{:?}", b.names());
    assert!(b.host.revenge.is_empty(), "the revenge ended");
    // A body without revenge: no Raid.
    let mut b = Bench::new(level99(&[A::RageRaid]), &[foe]);
    guarded_hit(&mut b, &foe);
    assert_eq!(b.sora.combat().xs.rage.raid_until, 0.0);
}

/// (ER90, XSI request 2) the guard's own counter prompt (Revenge Slash) no longer pushes Raid out of slot B in the step the
/// block offers it: Y gives Rage Raid, as Attack does; without Rage Raid (or a body without revenge) Y gives the counter.
#[test]
fn xsi_rage_raid_keeps_slot_b_so_y_gives_raid() {
    let foe = dummy(1, Vec3::new(150.0, 0.0, 0.0));
    let raid = X.rage_raid_command as i32;
    let mut b = Bench::new(level99(&[A::RageRaid]), &[foe]);
    b.host.revenge.push(TargetId(1));
    guarded_hit(&mut b, &foe);
    assert_eq!(b.sora.combat().situation.slot_b.map(|e| e.kind), Some(raid), "slot B holds Raid: {:?}", b.sora.combat().situation);
    b.step(&[PadButton::SITUATION]);
    b.until(60, &[], |_, s| s.combat().history.last() == Some(&xs::rage::RAGE_RAID));
    // The plain guard counter: slot B shows the guard's prompt (not Raid) and Y plays a counter, not Raid.
    for (abilities, revenge) in [(&[][..], true), (&[A::RageRaid][..], false)] {
        let mut b = Bench::new(level99(abilities), &[foe]);
        if revenge {
            b.host.revenge.push(TargetId(1));
        }
        guarded_hit(&mut b, &foe);
        let shown = b.sora.combat().situation.slot_b.map(|e| e.kind);
        assert!(shown.is_some() && shown != Some(raid), "{abilities:?} revenge {revenge}: slot B {shown:?}");
        b.step(&[PadButton::SITUATION]);
        b.run(30);
        assert!(!b.sora.combat().history.contains(&xs::rage::RAGE_RAID), "{:?}", b.kinds());
    }
}

#[test]
fn xsi_riskcharge_pays_hp_for_crimson_blast() {
    let mut b = Bench::new(level99(&[A::Riskcharge]), &[dummy(1, Vec3::new(200.0, 0.0, 0.0))]);
    b.lock(1);
    let max = b.sora.status().max_hp();
    b.run(10);
    assert!(!b.sora.combat().situation.has(xs::rage::RISKCHARGE_COMMAND), "not at full HP");
    b.sora.status_mut().set_hp(max / 5);
    b.run(30);
    assert!(b.sora.combat().situation.has(xs::rage::RISKCHARGE_COMMAND));
    let hp = b.sora.status().hp;
    b.step(&[PadButton::SITUATION]);
    b.run(2);
    assert_eq!(b.sora.combat().history.last(), Some(&xs::rage::RISKCHARGE), "{:?}", b.kinds());
    assert!(b.sora.status().damage_invalid);
    let cost = ((hp as f32 * X.riskcharge_cost).ceil() as i32).max(1);
    assert_eq!((b.sora.status().hp, b.sora.combat().xs.rage.level), ((hp - cost).max(1), 1));
    b.until(300, &[], |o, _| o.state != StateId::Attack);
    // The ground finisher is Crimson Blast now, its shot the level's row.
    b.combo(900, 1, |_| {}, |b| b.kinds().contains(&xs::finishers::CRIMSON_BLAST));
    assert!(b.kinds().contains(&xs::finishers::CRIMSON_BLAST), "{:?}", b.kinds());
    b.run(200);
    assert!(!b.hits_of("AN_CRIMSONBLAST_RC1").is_empty(), "{:?}", b.names());
    assert!(b.sora.status().hp >= 1);
}

// ------------------------------------------------------------------------------------ 2.15 Hidden Potential, 2.16 Break Boost

#[test]
fn xsi_hidden_potential_raises_a_low_level_soras_damage_on_a_powerful_foe() {
    let path = format!("{}/../../sheets/port/sora-loadout-99.json", env!("CARGO_MANIFEST_DIR"));
    let json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let first = |overcome: bool, powerful: bool| {
        let mut loadout = Loadout::new("sora-loadout-99");
        for row in json["rows"].as_array().unwrap() {
            loadout.set(row[0].as_str().unwrap(), &cell(&row[1]).unwrap()).unwrap();
        }
        loadout.set_level(10);
        loadout.resolve().unwrap();
        let mut status = Status::from_loadout(&loadout);
        let owned = status.abilities.owned(A::Overcome).max(1);
        status.abilities.set(A::Overcome, owned, if overcome { 1 } else { 0 });
        let mut b = Bench::new(status, &[dummy(1, Vec3::new(200.0, 0.0, 0.0))]);
        b.lock(1);
        if powerful {
            b.host.foe_levels.push((TargetId(1), 60));
        }
        b.step(&[PadButton::ATTACK]);
        b.run(60);
        b.hits().first().map_or(0.0, |h| h.damage)
    };
    let (plain, with) = (first(false, true), first(true, true));
    assert!(plain > 0.0 && with > plain, "{plain} -> {with}");
    assert_eq!(first(true, false), first(false, false), "an ordinary foe: no change");
    assert!(xs::overcome_floor(60) > xs::overcome_floor(10));
}

#[test]
fn xsi_break_boost_wears_a_strong_body_faster_and_marks_its_hits() {
    let run = |with: bool| {
        let mut b = Bench::new(level99(if with { &[A::BreakBoost] } else { &[] }), &[dummy(1, Vec3::new(200.0, 0.0, 0.0))]);
        strong(&mut b, 1);
        b.lock(1);
        b.combo(900, 1, |_| {}, |b| b.sora.combat().history.len() >= 7);
        b.run(120);
        let flagged = b.hits().iter().filter(|h| h.flags & xs::FLAG_BREAK_BOOST != 0).count();
        (1000 - b.host.endurance[0].1, flagged, b.sora.combat().xs.boosted, b.kinds())
    };
    let (plain, f0, _, k0) = run(false);
    let (boosted, f1, n, k1) = run(true);
    assert_eq!(k0, k1, "the same combo");
    assert_eq!(f0, 0);
    assert!(n >= 1 && f1 >= 1, "boosted hits {n}, flagged {f1}: {k1:?}");
    assert_eq!(boosted - plain, n as i32 * X.break_boost_rp as i32, "endurance taken {plain} -> {boosted}");
}
