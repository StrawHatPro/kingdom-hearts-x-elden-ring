//! (AUDIT-A ARROWGUN) Core traces of what AUDIT-A changed in the gun formchanges (build/sora_tmp/weapons/AUDIT-A/ARROWGUN.md):
//! - PlayAttack's +0x16C (0x44 .. 0x46): the stick steers the attack (Attack slot 131 0x141341870), MaxAcceleration back
//!   at End;
//! - ShooterMode: the actor's pitch from the aim (SetActorRotation(pitch, yaw, 0) 0x143EAD160) and back at End; the
//!   launchers' WEAPON GLOW while charging (pawn vt+0x1468 -> the weapon's m_CmnChargeEffect group 0x802 on its vfx_call,
//!   ended at the shot); the gate's finisher-end input (pawn vt+0x2748); EndStyle during ShooterMode = Fall + DoChain;
//! - Barrier Cracker's +0x234 |= 0x2800 (his push power until End: AUDIT-B W-CORE's reading); no guard-counter window in Double Arrowguns (0x80).

use kh_core::combat::forms::arrowgun;
use kh_core::enums::ETresDamageKind;
use kh_core::host::Call;
use kh_core::magic::View;
use kh_core::states::HitOnPlayer;
use kh_core::status::LoadoutValue;
use kh_core::test_world::TestWorld;
use kh_core::{Input, Loadout, Output, PadButton, PadButtons, PadState, Pose, RecordingHost, Sora, StateId, Status, TargetFlags, TargetId, TargetInfo, Tuning, Vec2, Vec3, SORA_PARAMS};

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

fn dummy(x: f32) -> TargetInfo {
    TargetInfo { id: TargetId(1), position: Vec3::new(x, 0.0, 0.0), center: Vec3::new(x, 0.0, 90.0), velocity: Vec3::ZERO, yaw: 180.0, radius: 40.0, half_height: 90.0, flags: TargetFlags(TargetFlags::ALIVE | TargetFlags::HITTABLE | TargetFlags::LOCKABLE | TargetFlags::ENEMY), hp: 1.0e6, hp_max: 1.0e6 }
}

struct Bench {
    sora: Sora,
    host: RecordingHost,
    world: TestWorld,
    held: PadButtons,
    stick: Vec2,
    log: Vec<Output>,
}

impl Bench {
    fn new(key: &str, at: f32) -> Bench {
        let origin = Vec3::new(0.0, 0.0, Sora::origin_height_on_floor(&SORA_PARAMS));
        let sora = Sora::new(SORA_PARAMS, Pose { position: origin, yaw: 0.0 }).with_tuning(Tuning::GAME_EXACT).with_status(holding(key));
        let mut host = RecordingHost::new();
        host.targets.push(dummy(at));
        let mut b = Bench { sora, host, world: TestWorld::floor(), held: PadButtons::NONE, stick: Vec2::ZERO, log: Vec::new() };
        b.idle(5);
        b
    }

    fn step(&mut self, held: &[PadButton]) -> Output {
        let set = held.iter().fold(PadButtons::NONE, |all, b| all.with(*b));
        let view = View::from_camera(Vec3::new(-400.0, 0.0, 200.0), 0.0, 0.0, 90.0, 1280, 720);
        let input = Input { move_stick: self.stick, view: Some(view), ..Input::default() }.with_pad(PadState::from_held(set, self.held));
        self.held = set;
        let (sora, world) = (&mut self.sora, &self.world);
        let out = self.host.step(world, |h| sora.step_hosted(DT, &input, h));
        self.log.push(out.clone());
        out
    }

    fn idle(&mut self, n: usize) {
        for _ in 0..n {
            self.step(&[]);
        }
    }

    fn transform(&mut self, cmd: i32) {
        self.sora.debug_add_style(100.0);
        self.idle(15);
        for _ in 0..40 {
            self.step(&[PadButton::SITUATION]);
            if self.step(&[]).state == StateId::Stylechange {
                break;
            }
        }
        for _ in 0..600 {
            if self.step(&[]).state != StateId::Stylechange {
                break;
            }
        }
        assert_eq!(self.sora.combat().form.cmd, cmd);
        self.idle(20);
    }

    /// The first attack of a combo with the stick at `stick`: the XY speeds while it plays.
    fn first_attack(&mut self, stick: Vec2) -> Vec<Vec3> {
        let mut out = Vec::new();
        self.step(&[PadButton::ATTACK]);
        self.idle(2);
        // the stick once the attack runs (with it held first he would run, not shoot)
        self.stick = stick;
        for _ in 0..60 {
            let o = self.step(&[]);
            if o.state == StateId::ComboDw {
                out.push(o.velocity);
            }
        }
        self.stick = Vec2::ZERO;
        self.idle(90);
        out
    }

    fn spawns_since(&self, from: usize) -> Vec<(kh_core::EffectHandle, String, String)> {
        self.host.calls[from..].iter().filter_map(|c| if let Call::Spawn(h, s) = c { Some((*h, s.name.to_string(), format!("{:?}", s.at))) } else { None }).collect()
    }
}

/// PlayAttack 0x44 writes +0x16C: slot 131 0x141341870 steers him with the stick (MySprintSpeed x the ramp toward the
/// stick's yaw); without the stick the speed halves each tick from MaxWalkSpeed x 0.9. 0x47 (the finisher) has no +0x16C.
#[test]
fn the_arrowguns_first_shots_steer_with_the_stick() {
    let mut b = Bench::new("WEP_KEYBLADE_SO_02", 250.0);
    b.transform(arrowgun::CHANGE_DW);
    let still = b.first_attack(Vec2::ZERO);
    assert!(!still.is_empty(), "0x44 ran");
    let mut b2 = Bench::new("WEP_KEYBLADE_SO_02", 250.0);
    b2.transform(arrowgun::CHANGE_DW);
    let steered = b2.first_attack(Vec2::new(1.0, 0.0));
    let side = |v: &[Vec3]| v.iter().map(|x| x.y.abs()).fold(0.0f32, f32::max);
    assert!(side(&still) < 50.0, "no stick: no sideways glide ({})", side(&still));
    assert!(side(&steered) > 300.0, "the stick to his side steers the attack sideways ({})", side(&steered));
}

/// ShooterMode in Magic Launcher: the right stick pitches the aim AND the actor (Output::pitch), the charge starts the
/// launcher's glow (vw_bom_lv0 on w_so024's vfx_call, group 0x802) and the shot ends it; End puts the pitch back.
#[test]
fn shootermode_pitches_the_actor_and_the_launcher_glows_while_charging() {
    let mut b = Bench::new("WEP_KEYBLADE_SO_02", 900.0);
    b.transform(arrowgun::CHANGE_DW);
    // the second stage
    b.sora.debug_add_style(100.0);
    b.idle(15);
    for _ in 0..40 {
        b.step(&[PadButton::SITUATION]);
        if b.step(&[]).state == StateId::Stylechange {
            break;
        }
    }
    for _ in 0..600 {
        if b.step(&[]).state != StateId::Stylechange {
            break;
        }
    }
    assert_eq!(b.sora.combat().form.cmd, arrowgun::CHANGE_BZ);
    b.idle(20);
    let from = b.host.calls.len();
    let mut pitched = 0.0f32;
    let mut entered = false;
    for _ in 0..240 {
        b.sora.set_shooter_look(Vec2::new(0.0, 1.0));
        let o = b.step(&[PadButton::RightTrigger]);
        entered |= o.state == StateId::Shootermode;
        pitched = if o.pitch.abs() > pitched.abs() { o.pitch } else { pitched };
    }
    assert!(entered, "ShooterMode");
    assert!(pitched.abs() > 10.0, "the actor pitches with the aim: {pitched}");
    let glow: Vec<_> = b.spawns_since(from).into_iter().filter(|s| s.1 == "vw_bom_lv0").collect();
    assert!(!glow.is_empty(), "the charge glow: {:?}", b.spawns_since(from).iter().map(|s| s.1.clone()).collect::<Vec<_>>());
    assert!(glow[0].2.contains("kb:w_so024:R_wep1_skt:vfx_call"), "on the launcher's vfx_call: {}", glow[0].2);
    let h = glow[0].0;
    assert!(b.host.calls[from..].iter().any(|c| matches!(c, Call::StopEffect(x, _) if *x == h)), "the shot ends the glow");
    b.sora.set_shooter_look(Vec2::ZERO);
    for _ in 0..200 {
        if b.step(&[]).state != StateId::Shootermode {
            break;
        }
    }
    b.idle(5);
    assert_eq!(b.log.last().map(|o| o.pitch), Some(0.0), "End: the pitch back (Turn((0, yaw, 0), 1.0))");
}

/// The gate (0x14142D9D0) refuses ShooterMode out of an attack after the finisher's end (pawn vt+0x2748 = pawn+0xE48
/// bit 5): RT held through 0x47 does not start it while COMBO_DW runs.
#[test]
fn no_shootermode_out_of_the_finisher() {
    let mut b = Bench::new("WEP_KEYBLADE_SO_02", 250.0);
    b.transform(arrowgun::CHANGE_DW);
    let mut in_finisher_shooter = false;
    let mut saw_finisher = false;
    for k in 0..400 {
        let finishing = b.sora.combat().finish_end;
        let buttons: &[PadButton] = if finishing { &[PadButton::RightTrigger] } else if k % 2 == 0 { &[PadButton::ATTACK] } else { &[] };
        let o = b.step(buttons);
        if finishing && o.state == StateId::ComboDw {
            saw_finisher = true;
        }
        if finishing && saw_finisher && o.state == StateId::Shootermode && b.log.iter().rev().nth(1).is_some_and(|p| p.state == StateId::ComboDw) {
            in_finisher_shooter = true;
        }
        if saw_finisher && !matches!(o.state, StateId::ComboDw | StateId::Shootermode) {
            break;
        }
    }
    assert!(saw_finisher, "0x47 ran with the finish end");
    assert!(!in_finisher_shooter, "ShooterMode did not cut into the finisher");
}

/// EndStyle during ShooterMode (0x14141CA20's tail): Fall(+0xA8) + DoChain: the next state is JUMP, ShooterModeEnd does not
/// play.
#[test]
fn the_end_of_the_form_during_shootermode_falls() {
    let mut b = Bench::new("WEP_KEYBLADE_SO_02", 900.0);
    b.transform(arrowgun::CHANGE_DW);
    for _ in 0..80 {
        b.step(&[PadButton::RightTrigger]);
    }
    assert_eq!(b.log.last().map(|o| o.state), Some(StateId::Shootermode));
    b.sora.combat_mut().form.time_left = 0.02;
    let mut after = Vec::new();
    for _ in 0..30 {
        let o = b.step(&[PadButton::RightTrigger]);
        after.push((o.state, o.anim.name.name()));
    }
    assert_eq!(b.sora.combat().form.cmd, 0, "the form ended");
    let first = after.iter().position(|a| a.0 != StateId::Shootermode).expect("ShooterMode left");
    assert_eq!(after[first].0, StateId::Jump, "Fall: {after:?}");
    assert!(!after[..first].iter().any(|a| a.1 == "2406"), "no ShooterModeEnd: {after:?}");
}

/// Barrier Cracker (0x25, Hunny Blasters): +0x234 |= 0x2800 at the play (AUDIT-B W-CORE: bit 11 = his push power PPL_90 until End); and
/// FUN_1414145C0 opens no guard-counter window in Double Arrowguns (0x80) while it does in Hunny Blasters (0x8E).
#[test]
fn barrier_cracker_raises_his_push_power_and_double_arrowguns_open_no_counter_window() {
    for (key, cmd, window) in [("WEP_KEYBLADE_SO_07", arrowgun::CHANGE_DW2, true), ("WEP_KEYBLADE_SO_02", arrowgun::CHANGE_DW, false)] {
        let mut b = Bench::new(key, 200.0);
        b.transform(cmd);
        b.sora.force_battle(true);
        b.sora.status_mut().abilities.set(kh_core::enums::ETresAbilityKind::ReflectGuard, 1, 1);
        b.idle(10);
        // the guard (Hunny: the Reflec guard; Shooting Star's neutral guard button is the Back Slide: hold the stick away)
        if cmd == arrowgun::CHANGE_DW {
            continue_guard_counter_free(&mut b);
            continue;
        }
        b.step(&[PadButton::GUARD]);
        b.idle(8);
        let me = b.log.last().unwrap().position;
        let at = Vec3::new(200.0, 0.0, 0.0);
        let d = (Vec3::new(me.x, me.y, 0.0) - at).normalize_or_zero();
        b.sora.take_hit(HitOnPlayer { direction: d, causer_yaw: d.y.atan2(d.x).to_degrees(), ..HitOnPlayer::new(ETresDamageKind::Small, 30.0, TargetId(1), at) });
        b.idle(1);
        assert_eq!(b.sora.actions().counter_window > 0.0, window, "{cmd:#x}: the counter window");
        let first = b.sora.combat().history.len();
        b.step(&[PadButton::ATTACK]);
        // (AUDIT-B W-CORE's correction of round 1: +0x234 |= 0x2800 is bit 11 = 0x1405B8B30(pawn+0x460, 8), his push power
        // PPL_90 until End, and bit 13 = End's plain air branch; not a hit flag)
        let mut raised = false;
        for _ in 0..4 {
            b.step(&[]);
            if b.sora.combat().history[first..].first() == Some(&0x25) {
                raised |= b.sora.actions().push_level.is_some();
                break;
            }
        }
        assert!(raised, "Barrier Cracker's play raises his push power (bit 11)");
        b.idle(120);
        assert_eq!(b.sora.actions().push_level, None, "End puts it back (0x1405B6100)");
    }
}

/// Double Arrowguns: no state of the guard button opens a counter window (0x1414145C0 returns for 0x80): a hit taken in
/// any state leaves it closed.
fn continue_guard_counter_free(b: &mut Bench) {
    let me = b.log.last().unwrap().position;
    let at = Vec3::new(200.0, 0.0, 0.0);
    let d = (Vec3::new(me.x, me.y, 0.0) - at).normalize_or_zero();
    b.sora.take_hit(HitOnPlayer { direction: d, causer_yaw: d.y.atan2(d.x).to_degrees(), ..HitOnPlayer::new(ETresDamageKind::Small, 30.0, TargetId(1), at) });
    b.idle(1);
    assert_eq!(b.sora.actions().counter_window, 0.0, "0x80: no counter window");
}

/// (AUDIT-A ARROWGUN) PlayAttack 0x48 (+0x16D = 1, +0x188 = 500): Decide picks it when the target is more below him than
/// away (dz < -100 and dz < -dist; 0x141361270); the traction (slot 116 0x141349CC0 line 116: dz = target - him + the
/// offset, clamped +-500) then aims at a point 500 cm ABOVE the target, so the Z channel (DirZ = dz x gain while the signs
/// agree) dives at (target + 500 - him) instead of the whole drop; when the Z window closes (+0x16D) DirZ and the offset go
/// to 0 and he falls under gravity. The rig (kh_core::test_world): a 700 cm ledge under him while he takes the form, then
/// gone (he is in the air ~500 cm above the dummy's centre, 470 cm away on the floor), locked on the dummy: the first shot
/// of the combo is 0x48; its Z speed while the window is open is (target centre + 500 - his centre) x 1.0, not the -500
/// clamp of the whole drop; after the window he falls.
#[test]
fn the_arrowguns_downward_shot_aims_500_above_the_target() {
    // a ledge 700 cm high under him (the dummy stands on the floor beyond its edge); the form taken up there
    let origin = Vec3::new(0.0, 0.0, 700.0 + Sora::origin_height_on_floor(&SORA_PARAMS));
    let sora = Sora::new(SORA_PARAMS, Pose { position: origin, yaw: 0.0 }).with_tuning(Tuning::GAME_EXACT).with_status(holding("WEP_KEYBLADE_SO_02"));
    let mut host = RecordingHost::new();
    host.targets.push(dummy(300.0));
    let ledge = TestWorld::floor().with_box(Vec3::new(-200.0, -200.0, 0.0), Vec3::new(200.0, 200.0, 700.0));
    let mut b = Bench { sora, host, world: ledge, held: PadButtons::NONE, stick: Vec2::ZERO, log: Vec::new() };
    b.idle(5);
    b.transform(arrowgun::CHANGE_DW);
    // the ledge gone: he is in the air, 700 cm above the dummy's floor; locked on it (R1: the soft search keeps to his height band)
    b.world = TestWorld::floor();
    b.sora.combat_mut().lockon.current = TargetId(1);
    b.idle(1);
    let first = b.sora.combat().history.len();
    let mut trace = Vec::new();
    b.step(&[PadButton::ATTACK]);
    for _ in 0..90 {
        let o = b.step(&[]);
        let a = b.sora.attack();
        trace.push((o.state, o.position.z, o.velocity.z, a.map(|a| (a.kind, a.enable_traction, a.dz, a.dist, a.target.0))));
        if o.state != StateId::ComboDw && trace.len() > 5 {
            break;
        }
    }
    let kinds = b.sora.combat().history[first..].to_vec();
    assert_eq!(kinds.first(), Some(&0x48), "the downward shot: {kinds:x?} {trace:?}");
    // the Z window: the Z speed follows (target centre + 500 - his centre); the drop itself is ~700 cm
    let open: Vec<_> = trace.iter().filter(|t| t.3.is_some_and(|a| a.0 == 0x48 && a.1)).collect();
    assert!(!open.is_empty(), "0x48's traction window opened: {trace:?}");
    // DirZ = (target centre 90 + 500 - his centre) x z gain 1.0 (cm/s), every tick of the window: ~ -2 here, where the
    // drop itself (dz, Decide's -502) would dive at the -500 clamp
    for t in &open {
        let aimed = 90.0 + 500.0 - t.1;
        assert!((t.2 - aimed).abs() < 1.0, "the Z channel aims 500 cm above the target: vz {} at z {} (aimed {aimed}; {trace:?})", t.2, t.1);
    }
    assert!(open.iter().all(|t| t.3.is_some_and(|a| a.2 < -400.0)), "Decide's dz is the whole drop");
    // the window closed: DirZ 0 (+0x16D), he falls under gravity (the speed grows downward)
    let after: Vec<f32> = trace.iter().skip_while(|t| !t.3.is_some_and(|a| a.1)).skip_while(|t| t.3.is_some_and(|a| a.1)).map(|t| t.2).collect();
    assert!(after.len() > 3 && after[3] < after[0], "after the window he falls: {after:?}");
    let mut text = String::from("# (AUDIT-A ARROWGUN) 0x48 (+0x16D / +0x188 = 500): step | state | his centre z | vz | kind, traction window, Decide dz, dist, target | aimed vz = 590 - z
");
    for (k, t) in trace.iter().enumerate() {
        text.push_str(&format!("{k:3} | {:?} | {:.2} | {:.2} | {:?} | {:.2}
", t.0, t.1, t.2, t.3, 590.0 - t.1));
    }
    std::fs::write(format!("{}/../../build/sora_tmp/weapons/AUDIT-A/logs/audit_a_arrowgun_0x48.trace.txt", env!("CARGO_MANIFEST_DIR")), text).ok();
}

/// (AUDIT-A ARROWGUN) The command begin / end events of a finish (0x14150A1F0 / 0x14150C440) call 0x14140DF80: the DARK
/// SCREEN (the scene manager's master light x MyDarkScreenRate 0.5 over 1 s, controller+0x878 bit 2) on while FINISH_BZ runs
/// and off with its end.
#[test]
fn finish_bz_darkens_the_screen_while_it_runs() {
    let mut b = Bench::new("WEP_KEYBLADE_SO_02", 300.0);
    b.transform(arrowgun::CHANGE_DW);
    b.sora.debug_add_style(100.0);
    b.idle(15);
    for _ in 0..40 {
        b.step(&[PadButton::SITUATION]);
        if b.step(&[]).state == StateId::Stylechange {
            break;
        }
    }
    for _ in 0..600 {
        if b.step(&[]).state != StateId::Stylechange {
            break;
        }
    }
    assert_eq!(b.sora.combat().form.cmd, arrowgun::CHANGE_BZ);
    b.idle(30);
    assert!(!b.sora.combat().form.dark_screen, "no dark screen before the finish");
    b.step(&[PadButton::SITUATION]);
    let mut during = Vec::new();
    for _ in 0..600 {
        let o = b.step(&[]);
        if o.state == StateId::Stylefinish {
            during.push(b.sora.combat().form.dark_screen);
        } else if !during.is_empty() {
            break;
        }
    }
    assert!(!during.is_empty() && during.iter().all(|d| *d), "the dark screen is on through FINISH_BZ: {during:?}");
    assert!(!b.sora.combat().form.dark_screen, "the end event puts it off");
    assert_eq!((kh_core::combat::form::DARK_SCREEN_RATE, kh_core::combat::form::DARK_SCREEN_TIME), (0.5, 1.0));
}
