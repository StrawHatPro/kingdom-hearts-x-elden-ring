//! (F-ARROWGUN) Double Arrowguns -> Magic Launcher (Shooting Star, CHANGE_DW 0x80 -> CHANGE_BZ 0x83) and Hunny Blasters ->
//! Hunny Launcher (Hunny Spout, 0x8E -> 0x8F): the trace of each chain from the offer to its end
//! (build/sora_tmp/weapons/F-ARROWGUN/STATE.md): offer -> STYLECHANGE (StyleChange2 / 16, the wear form, the guns) -> the
//! COMBO_DW combo (0x44 0x45 0x46 0x47, curve / spin / spiral bullets that home and hit) -> the second stage's offer
//! inside the first -> STYLECHANGE (StyleChange5 / 17) -> the COMBO_BZ combo (0x4A 0x4B 0x4C swings, the far bomb shots
//! 0x4D 0x4E and their explosions) -> FINISH_BZ (M_5417, BZ_BIG_BANG -> BigBangEx) -> the form's end.

use kh_core::combat::forms::arrowgun;
use kh_core::host::EventKind;
use kh_core::status::LoadoutValue;
use kh_core::test_world::TestWorld;
use kh_core::{Input, Loadout, Output, PadButton, PadButtons, PadState, Pose, RecordingHost, Sora, StateId, Status, TargetFlags, TargetId, TargetInfo, Tuning, Vec3, SORA_PARAMS};

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

fn holding(key: &str) -> Status {
    let path = format!("{}/../../sheets/port/sora-loadout-99.json", env!("CARGO_MANIFEST_DIR"));
    let json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))).unwrap();
    let mut loadout = Loadout::new("sora-loadout-99");
    for row in json["rows"].as_array().unwrap() {
        let k = row[0].as_str().unwrap();
        let value = if k == "abilities.equipped" { LoadoutValue::List(Vec::new()) } else { cell(&row[1]).unwrap() };
        loadout.set(k, &value).unwrap_or_else(|e| panic!("{e}"));
    }
    loadout.resolve().unwrap();
    let mut s = Status::from_loadout(&loadout);
    s.weapon = key.to_string();
    s
}

fn dummy(id: u32, x: f32) -> TargetInfo {
    TargetInfo { id: TargetId(id), position: Vec3::new(x, 0.0, 0.0), center: Vec3::new(x, 0.0, 90.0), velocity: Vec3::ZERO, yaw: 180.0, radius: 40.0, half_height: 90.0, flags: TargetFlags(TargetFlags::ALIVE | TargetFlags::HITTABLE | TargetFlags::LOCKABLE | TargetFlags::ENEMY), hp: 1.0e6, hp_max: 1.0e6 }
}

struct Bench {
    sora: Sora,
    host: RecordingHost,
    world: TestWorld,
    log: Vec<(StateId, u8, &'static str, Vec<EventKind>)>,
}

impl Bench {
    fn new(key: &str, at: f32) -> Bench {
        let origin = Vec3::new(0.0, 0.0, Sora::origin_height_on_floor(&SORA_PARAMS));
        let sora = Sora::new(SORA_PARAMS, Pose { position: origin, yaw: 0.0 }).with_tuning(Tuning::GAME_EXACT).with_status(holding(key));
        let mut host = RecordingHost::new();
        host.targets.push(dummy(1, at));
        let mut b = Bench { sora, host, world: TestWorld::floor(), log: Vec::new() };
        b.idle(5);
        b
    }

    fn step(&mut self, held: &[PadButton]) -> Output {
        let set = held.iter().fold(PadButtons::NONE, |all, b| all.with(*b));
        let input = Input::default().with_pad(PadState::from_held(set, PadButtons::NONE));
        let (sora, world) = (&mut self.sora, &self.world);
        let out = self.host.step(world, |h| sora.step_hosted(DT, &input, h));
        self.log.push((out.state, out.sub_step, out.anim.name.name(), out.events.iter().map(|e| e.kind).collect()));
        out
    }

    fn idle(&mut self, n: usize) {
        for _ in 0..n {
            self.step(&[]);
        }
    }

    fn clips(&self, from: usize) -> Vec<&'static str> {
        let mut out: Vec<&'static str> = Vec::new();
        for (_, _, clip, _) in &self.log[from..] {
            if out.last() != Some(clip) {
                out.push(clip);
            }
        }
        out
    }

    /// Fill the gauge, take the offer `cmd` with Triangle, run STYLECHANGE; returns the step the form began at.
    fn transform(&mut self, cmd: i32) -> usize {
        self.sora.debug_add_style(100.0);
        self.idle(15);
        let list: Vec<i32> = self.sora.combat().situation.list.iter().map(|e| e.kind).collect();
        assert!(list.contains(&cmd), "{cmd:#x} is offered: {list:x?}");
        let from = self.log.len();
        for _ in 0..40 {
            self.step(&[PadButton::SITUATION]);
            if self.step(&[]).state == StateId::Stylechange {
                break;
            }
        }
        for _ in 0..600 {
            if self.step(&[]).state != StateId::Stylechange {
                return from;
            }
        }
        panic!("STYLECHANGE did not end");
    }

    /// Attack presses until the chain ends; the kinds played.
    fn combo(&mut self) -> Vec<u8> {
        let first = self.sora.combat().history.len();
        // Attack on every other step while a gun attack runs (the first press starts it); stop when the chain is over.
        let (mut attacking, mut finished) = (false, false);
        for k in 0..900 {
            // No press once the finisher began (its finish end): a press after the chain's end would start a new one.
            finished |= self.sora.combat().finish_end;
            let press = k % 2 == 0 && (k < 2 || attacking) && !finished;
            let out = if press { self.step(&[PadButton::ATTACK]) } else { self.step(&[]) };
            attacking = matches!(out.state, StateId::ComboDw | StateId::ComboBz | StateId::Attack);
            if k > 30 && !attacking && self.sora.combat().history.len() > first {
                break;
            }
        }
        self.sora.combat().history[first..].to_vec()
    }

    fn hits(&self) -> usize {
        self.host.hits().len()
    }
}

fn chain(key: &str, dw: i32, bz: i32, dw_clip: &str, bz_clip: &str) {
    let mut b = Bench::new(key, 250.0);
    assert_eq!(kh_core::combat::forms::moveset_name("form.DW"), "arrowgun");
    // ---- the first stage
    let from = b.transform(dw);
    let f = b.sora.combat().form;
    assert_eq!((f.cmd, f.level), (dw, 1));
    assert!(b.clips(from).contains(&dw_clip), "{:?}", b.clips(from));
    assert_eq!(f.weapon_units().len(), 2, "two guns: {:?}", f.weapon_units());
    b.idle(30);
    let (from, hits) = (b.log.len(), b.hits());
    let fired = b.sora.combat().arrowgun.fired;
    let kinds = b.combo();
    // The finisher 0x47 sets the finish end (pawn vt+0x2740(1)): CanAttack refuses a step after it.
    assert_eq!(kinds, [0x44, 0x45, 0x46, 0x47], "COMBO_DW near one target");
    assert!(b.log[from..].iter().any(|l| l.0 == StateId::ComboDw), "the attack class is COMBO_DW");
    for c in ["2411", "2412", "2413", "2414"] {
        assert!(b.clips(from).contains(&c), "{c}: {:?}", b.clips(from));
    }
    b.idle(120);
    let ag = &b.sora.combat().arrowgun;
    assert_eq!(ag.fired - fired, 6 + 8 + 8 + 16, "the bullets: 0x44 6 notifies x 1, 0x45 4 x 2, 0x46 4 x 2, 0x47 2 hands x a ring of 8; history {:x?}, clips {:?}", b.sora.combat().history, b.clips(from));
    assert!(b.hits() > hits + 10, "the bullets hit the target: {} hits", b.hits() - hits);
    assert!(b.host.hits()[hits..].iter().any(|(r, _)| r.attack_name.ends_with("_SHOT")) && b.host.hits()[hits..].iter().any(|(r, _)| r.attack_name.ends_with("_FINISH")), "the shot and the finisher rows");
    // m_MainLife 5 s: a bullet of the ring that missed flies on until then.
    b.idle(300);
    assert!(b.sora.combat().arrowgun.list.is_empty(), "every bullet has landed or expired");
    // ---- the second stage, offered inside the first
    let from = b.transform(bz);
    let f = b.sora.combat().form;
    assert_eq!((f.cmd, f.level), (bz, 2));
    assert!(b.clips(from).contains(&bz_clip), "{:?}", b.clips(from));
    b.idle(30);
    let (from, hits) = (b.log.len(), b.hits());
    assert_eq!(b.combo(), [0x4A, 0x4B, 0x4C], "COMBO_BZ near: the swings");
    assert!(b.log[from..].iter().any(|l| l.0 == StateId::ComboBz));
    assert!(b.hits() > hits, "the launcher's swings hit (its StickM / StickL / EnergyRelease shapes)");
    if bz == arrowgun::CHANGE_BZ2 {
        assert!(b.host.hits()[hits..].iter().any(|(r, _)| r.attack_name.starts_with("BZ2_")), "Hunny Launcher hits with the honey rows (0x141356DC0)");
    }
    // Far away: the bomb shots and their explosions.
    b.idle(60);
    b.host.targets[0].position.x = 1200.0;
    b.host.targets[0].center.x = 1200.0;
    let respawned = b.sora.combat().arrowgun.respawned;
    assert_eq!(b.combo()[0], 0x4D, "far: the bomb shot");
    b.idle(240);
    assert!(b.sora.combat().arrowgun.respawned > respawned, "BombShot1 became its explosion");
    // ---- FINISH_BZ
    assert_eq!(b.sora.combat().situation.slot_b.map(|e| e.kind), Some(arrowgun::FINISH_BZ), "FINISH_BZ on offer");
    let from = b.log.len();
    let respawned = b.sora.combat().arrowgun.respawned;
    b.step(&[PadButton::SITUATION]);
    for _ in 0..600 {
        if b.step(&[]).state != StateId::Stylefinish && b.log.len() > from + 5 {
            break;
        }
    }
    assert!(b.clips(from).contains(&"5417"), "{:?}", b.clips(from));
    b.idle(200);
    assert!(b.sora.combat().arrowgun.respawned > respawned, "BZ_BIG_BANG became BigBangEx");
    assert_eq!(b.sora.combat().form.cmd, 0, "the finish ends the form");
}

#[test]
fn shooting_star_double_arrowguns_to_magic_launcher() {
    chain("WEP_KEYBLADE_SO_02", arrowgun::CHANGE_DW, arrowgun::CHANGE_BZ, "StyleChange2", "StyleChange5");
}

#[test]
fn hunny_spout_hunny_blasters_to_hunny_launcher() {
    chain("WEP_KEYBLADE_SO_07", arrowgun::CHANGE_DW2, arrowgun::CHANGE_BZ2, "StyleChange16", "StyleChange17");
}

impl Bench {
    /// One step with the stick and the guard button pressed this frame.
    fn guard_press(&mut self, stick: kh_core::Vec2) -> Output {
        let pressed = PadButtons::NONE.with(PadButton::GUARD);
        let input = Input { move_stick: stick, ..Input::default() }.with_pad(PadState { held: pressed, pressed, released: PadButtons::NONE });
        let (sora, world) = (&mut self.sora, &self.world);
        let out = self.host.step(world, |h| sora.step_hosted(DT, &input, h));
        self.log.push((out.state, out.sub_step, out.anim.name.name(), out.events.iter().map(|e| e.kind).collect()));
        out
    }
}

/// (AirSlide 0x141332930 / 0x141340A10 / 0x141343E10) Double Arrowguns in battle: the guard button with the stick is the
/// Sliding Shot (the clip by the stick relative to his facing, a DW_NORMAL pair per TresShot), without it the Back Slide
/// (clip 2062, it shoots too); Magic Launcher's neutral Back Slide (5058) does not shoot. Hunny Blasters guard with the Reflec
/// guard (AnimSet16 G279).
#[test]
fn sliding_shot_back_slide_and_the_hunny_reflec_guard() {
    let mut b = Bench::new("WEP_KEYBLADE_SO_02", 600.0);
    b.transform(arrowgun::CHANGE_DW);
    b.sora.force_battle(true);
    b.idle(30);
    // Stick to his right (yaw 0 = +x; the stick's x is right): SlideShotR.
    let fired = b.sora.combat().arrowgun.fired;
    let from = b.log.len();
    let out = b.guard_press(kh_core::Vec2::new(1.0, 0.0));
    assert_eq!(out.state, StateId::AirSlide, "the Sliding Shot is an AIR_SLIDE");
    b.idle(60);
    let clips = b.clips(from);
    assert!(clips.iter().any(|c| ["2061", "2062", "2063", "2064"].contains(c)), "a SlideShot clip: {clips:?}");
    assert_eq!(b.sora.combat().arrowgun.fired - fired, 4, "two TresShot notifies x a DW_NORMAL pair");
    // Neutral stick: the Back Slide (2062), shooting as well (flag 4 in battle).
    b.idle(30);
    let (fired, from) = (b.sora.combat().arrowgun.fired, b.log.len());
    assert_eq!(b.guard_press(kh_core::Vec2::ZERO).state, StateId::AirSlide);
    b.idle(60);
    assert!(b.clips(from).contains(&"2062"), "{:?}", b.clips(from));
    assert_eq!(b.sora.combat().arrowgun.fired - fired, 4);
    // ---- Hunny Blasters: the guard is the Reflec guard (G279); no Back Slide without the stick.
    let mut b = Bench::new("WEP_KEYBLADE_SO_07", 600.0);
    b.transform(arrowgun::CHANGE_DW2);
    b.sora.force_battle(true);
    // The guard itself needs REFLECT_GUARD (the bench's loadout equips none).
    b.sora.status_mut().abilities.set(kh_core::enums::ETresAbilityKind::ReflectGuard, 1, 1);
    b.idle(30);
    let from = b.log.len();
    b.guard_press(kh_core::Vec2::ZERO);
    b.idle(40);
    assert!(b.log[from..].iter().any(|l| l.0 == StateId::Guard) && !b.log[from..].iter().any(|l| l.0 == StateId::AirSlide), "neutral: a guard, not a Back Slide: {:?}", b.log[from..].iter().map(|l| l.0).collect::<std::collections::BTreeSet<_>>());
    assert!(b.clips(from).contains(&"G279"), "the Reflec guard: {:?}", b.clips(from));
}
