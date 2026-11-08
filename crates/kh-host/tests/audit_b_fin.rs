//! (AUDIT-B W-FIN) The finishers' last open items, end to end through the real core and kh-host's rig.
//!
//! `spear_*`: HIGHWIND'S SPEAR STICKS TO THE HIT BONE. KH3's ATresProjectileBase 0x141473E30 (slot 537, m_bAttachToHitActor,
//! TresProjHL_Spear) attaches the spear to the hit's component (its master-pose component when it has one) at the hit's
//! FHitResult.BoneName (+0x78) with KeepWorldTransform (FUN_143E8FC60, DAT_148750ED0), then adds m_AttachToHitActorOffset
//! (0 in every blueprint). The port: kh-core `ShotStick::at_hit` / `follow` over the host seam `Targets::hit_bone` /
//! `bone_pose`; a host without bones keeps the actor's origin and yaw. The rig's dummy carries one bone here (kh-host
//! `rig::Dummy::bone`), which the test moves and turns as an animated skeleton would.
//! Skipped, saying so, without the pack.

use glam::{Quat, Vec3};
use kh_core::host::BonePose;
use kh_core::{PadButton, StateId};
use kh_host::audit_a::Session;

/// Wheel of Fate's models: the hand keyblade, the ship (transform unit), the spear, the flag, the Kraken.
const MODELS: [&str; 5] = ["w_so090", "w_so091", "w_so092", "w_so093", "w_so094"];

fn idle(s: &mut Session, n: usize) {
    for _ in 0..n {
        s.step(&[]);
    }
}

/// Highwind (the gauge full, Triangle, StyleChange14 to its end), then COMBO_HL near the dummy to its spear rain.
fn highwind_rain(s: &mut Session) {
    idle(s, 5);
    s.rig.sora.debug_add_style(100.0);
    idle(s, 20);
    assert!(s.rig.sora.combat().situation.has(0x8C), "Highwind offered");
    s.step(&[PadButton::SITUATION]);
    let mut done = false;
    for _ in 0..600 {
        if s.step(&[]) != StateId::Stylechange && s.rig.sora.combat().form.cmd == 0x8C {
            done = true;
            break;
        }
    }
    assert!(done, "StyleChange14 ended in Highwind");
    idle(s, 30);
    let o = s.rig.out.clone();
    let a = o.yaw.to_radians();
    let at = o.position + Vec3::new(a.cos(), a.sin(), 0.0) * 200.0;
    if let Some(d) = s.rig.dummy.as_mut() {
        (d.info.position, d.info.center) = (Vec3::new(at.x, at.y, 0.0), Vec3::new(at.x, at.y, 90.0));
    }
    let spawned = s.rig.sora.combat().shots_spawned;
    for k in 0..1200 {
        let state = if k % 2 == 0 { s.step(&[PadButton::ATTACK]) } else { s.step(&[]) };
        if k > 30 && matches!(state, StateId::Idle | StateId::Run | StateId::Jump) {
            break;
        }
    }
    assert!(s.rig.sora.combat().shots_spawned - spawned >= 15, "the spear rain fell");
}

/// The spears armed to stick and not stuck yet (standing in the floor): their centres.
fn armed(s: &Session) -> Vec<Vec3> {
    s.rig.sora.combat().shots.iter().filter(|x| x.name == "HL_SPEAR" && x.stick.is_some_and(|k| !k.stuck())).map(|x| x.centre()).collect()
}

/// The stuck spears: (centre, yaw, on a bone).
fn stuck(s: &Session) -> Vec<(Vec3, f32, bool)> {
    s.rig.sora.combat().shots.iter().filter_map(|x| x.stick.filter(|k| k.stuck()).map(|k| (x.position, x.yaw, k.bone.is_some()))).collect()
}

/// The dummy stepped onto the spear standing at `spear` (its feet under it), with `bone` as its skeleton.
fn onto(s: &mut Session, spear: Vec3, bone: Option<BonePose>) {
    let d = s.rig.dummy.as_mut().expect("the dummy");
    (d.info.position, d.info.center) = (Vec3::new(spear.x, spear.y, 0.0), Vec3::new(spear.x, spear.y, 90.0));
    d.bone = bone;
}

/// A host WITH bones: the spear sticks on the hit's bone keeping its world transform (KeepWorldTransform), then follows
/// that bone's place AND turn every step (the bone moved off the actor's origin and turned about a tilted axis: the
/// origin + yaw rule would put it elsewhere); the actor's own origin moving without its bone does not carry it.
#[test]
fn spear_sticks_on_the_hit_bone_and_follows_it() {
    let Some(mut s) = Session::new("WEP_KEYBLADE_SO_09", &MODELS, "w_so090", Some(Vec3::new(200.0, 0.0, 0.0)), "auditb_fin_spear_bone") else { return };
    highwind_rain(&mut s);
    let spear = *armed(&s).first().expect("a spear standing in the floor, armed to stick");
    // the bone: the dummy's spine 60 cm up from its feet, turned 30 degrees about Z
    let b0 = BonePose { position: Vec3::new(spear.x, spear.y, 60.0), rotation: Quat::from_rotation_z(30f32.to_radians()) };
    onto(&mut s, spear, Some(b0));
    s.step(&[]);
    let now = stuck(&s);
    assert_eq!(now.len(), 1, "the spear stuck in the dummy: {now:?}");
    let (at0, yaw0, on_bone) = now[0];
    assert!(on_bone, "stuck ON THE HIT BONE (the host has bones)");
    assert!((at0 - spear).length() < 1.0e-2, "KeepWorldTransform: it stays where it hit, {spear} -> {at0}");
    let local = b0.rotation.inverse() * (at0 - b0.position);
    // the bone moves 2 m and turns (90 degrees about a tilted axis): the spear rides on it
    let b1 = BonePose { position: b0.position + Vec3::new(120.0, -160.0, 40.0), rotation: Quat::from_axis_angle(Vec3::new(1.0, 0.0, 1.0).normalize(), 90f32.to_radians()) * b0.rotation };
    s.rig.dummy.as_mut().unwrap().bone = Some(b1);
    s.step(&[]);
    let want = b1.position + b1.rotation * local;
    let (at1, yaw1, _) = stuck(&s)[0];
    assert!((at1 - want).length() < 1.0e-2, "on the bone: {at1} (want {want})");
    let fwd = (b1.rotation * b0.rotation.inverse() * Quat::from_rotation_z(yaw0.to_radians())) * Vec3::X;
    let want_yaw = fwd.y.atan2(fwd.x).to_degrees();
    assert!((yaw1 - want_yaw).abs() < 1.0e-2, "its turn follows the bone's: yaw {yaw1} (want {want_yaw})");
    let origin_rule = {
        let d = s.rig.dummy.unwrap().info;
        let (sin, cos) = d.yaw.to_radians().sin_cos();
        let l = at0 - Vec3::new(spear.x, spear.y, 0.0);
        let back = Vec3::new(l.x * cos + l.y * sin, -l.x * sin + l.y * cos, l.z);
        d.position + Vec3::new(back.x * cos - back.y * sin, back.x * sin + back.y * cos, back.z)
    };
    assert!((at1 - origin_rule).length() > 100.0, "not the origin + yaw rule");
    // the actor's origin moves, its bone stays: the spear stays with the bone
    let d = s.rig.dummy.as_mut().unwrap();
    (d.info.position, d.info.center) = (d.info.position + Vec3::Y * 300.0, d.info.center + Vec3::Y * 300.0);
    s.step(&[]);
    assert!((stuck(&s)[0].0 - want).length() < 1.0e-2, "the bone, not the origin, carries it");
    std::fs::write(
        kh_host::audit_a::project().join("build").join("sora_tmp").join("weapons").join("AUDIT-B").join("W-FIN").join("traces").join("spear_bone.trace"),
        format!("spear at hit {spear}\nstuck {at0} yaw {yaw0} on bone {on_bone}\nbone 0 {b0:?}\nbone 1 {b1:?}\nfollowed {at1} yaw {yaw1} (want {want} / {want_yaw})\norigin+yaw rule would be {origin_rule}\n"),
    )
    .ok();
}

/// A host WITHOUT bones (the dummy's `bone` None, `Targets::hit_bone` None): the spear follows the actor's origin and
/// yaw as before (F-POLISH's rule, the fallback).
#[test]
fn spear_without_bones_follows_the_origin_and_yaw() {
    let Some(mut s) = Session::new("WEP_KEYBLADE_SO_09", &MODELS, "w_so090", Some(Vec3::new(200.0, 0.0, 0.0)), "auditb_fin_spear_origin") else { return };
    highwind_rain(&mut s);
    let spear = *armed(&s).first().expect("a spear standing in the floor, armed to stick");
    onto(&mut s, spear, None);
    s.step(&[]);
    let now = stuck(&s);
    assert_eq!(now.len(), 1, "the spear stuck in the dummy: {now:?}");
    let (at0, yaw0, on_bone) = now[0];
    assert!(!on_bone, "no bone: the origin and yaw");
    // the actor moves 3 m and turns 90 degrees: the spear is carried about its origin
    let d = s.rig.dummy.as_mut().unwrap();
    let (p0, y0) = (d.info.position, d.info.yaw);
    (d.info.position, d.info.center, d.info.yaw) = (p0 + Vec3::Y * 300.0, d.info.center + Vec3::Y * 300.0, y0 + 90.0);
    s.step(&[]);
    let l = at0 - p0;
    let want = p0 + Vec3::Y * 300.0 + Vec3::new(-l.y, l.x, l.z);
    let (at1, yaw1, _) = *stuck(&s).first().expect("still stuck");
    assert!((at1 - want).length() < 1.0e-2, "carried on the origin and yaw: {at1} (want {want})");
    assert!((yaw1 - yaw0).abs() < 1.0e-4, "its yaw unchanged without a bone");
}

// ================================================================================================ (AUDIT-B W-FIN) the ceiling look
//
// `ceiling_*`: FINISH_SK'S CEILING LOOK (BP_TresPlayerStateEventCamera_fz_key ATARI: 7419's line trace 957 cm over his head
// hits; 7423's Param-1 start -> Fz_FInish01_End @10761 .. @9451 .. @6975; 7423's end -> Fz_FInish02_End @10776 .. @10151 ..
// @7243). The particle vw_so073_fin_fsg0 is the core's (kh-core states/bp_event.rs `ceiling_look`), the level hide / lights /
// post-process the renderer's from the camera stack's `FormBpOutput::tower` (kh-er-render bluehour.rs `set_tower`).

/// Crystal Snow's models: the hand keyblade, the transform unit, the claws, the blades, the Chandelier.
const CS_MODELS: [&str; 9] = ["w_so080", "w_so081", "w_so082", "w_so083", "w_so084", "w_so085", "w_so086", "w_so087", "w_so088"];

/// One spawn of the particle: (step, handle, attach, his actor location, systems drawn).
type CeilingSpawn = (u64, u32, String, Vec3, usize);

/// What one FINISH_SK run showed: 7423's BpEvents (step, begin, param), the camera stack's `tower` edges, the particle's
/// spawns and its stops (step, handle, immediate).
#[derive(Default)]
struct CeilingRun {
    bp: Vec<(u64, bool, i32)>,
    tower: Vec<(u64, bool)>,
    spawns: Vec<CeilingSpawn>,
    stops: Vec<(u64, u32, bool)>,
    log: Vec<String>,
    handle: Option<u32>,
}

/// Crystal Snow with the port's camera blueprints (kh-camera FormBpCameras, stepped on the same events and world as kh-er
/// cam.rs steps them).
struct CeilingRig {
    s: Session,
    cams: kh_camera::form_bp::FormBpCameras,
    run: CeilingRun,
}

impl CeilingRig {
    fn step(&mut self, held: &[PadButton]) -> StateId {
        use kh_camera::camera_anim::AnimView;
        use kh_camera::form_bp::{FormBpFrame, PerfCamera};
        let state = self.s.step(held);
        let out = self.s.rig.out.clone();
        for e in &out.events {
            if let kh_core::host::EventKind::AnimBpEvent { seq: "7423", param, begin } = e.kind {
                self.run.bp.push((out.step, begin, param));
            }
        }
        let field = AnimView { location: out.position + Vec3::new(-400.0, 0.0, 200.0), rotation: kh_camera::Rot::new(-10.0, 0.0, 0.0), fov: 67.0 };
        let frame = FormBpFrame { sora: (out.position, out.yaw), field, state: out.state, ultima: false, perf: PerfCamera::default() };
        let o = self.cams.step(1.0 / 60.0, &out.events, &frame, Some(&self.s.rig.world));
        if let Some(on) = o.tower {
            self.run.tower.push((out.step, on));
        }
        let step = out.step;
        self.run.log.extend(self.cams.claws.log.drain(..).map(|l| format!("step {step} fz_key: {l}")));
        let row = self.s.rows.last().expect("a row");
        for f in &row.effects {
            if f.stop.is_none() && f.name == kh_core::states::bp_event::CEILING_PARTICLE {
                self.run.handle = Some(f.handle);
                self.run.spawns.push((step, f.handle, f.attach.clone(), out.position, f.drawn));
            }
            if let (Some(immediate), Some(h)) = (f.stop, self.run.handle) {
                if f.handle == h {
                    self.run.stops.push((step, f.handle, immediate));
                }
            }
        }
        state
    }

    fn idle(&mut self, n: usize) {
        for _ in 0..n {
            self.step(&[]);
        }
    }
}

/// Blizzard Claws -> Blizzard Blades -> FINISH_SK with the dummy near. `ceiling`: a slab 5 m over the floor.
fn finish_sk_run(ceiling: bool, name: &str) -> Option<CeilingRun> {
    let mut s = Session::new("WEP_KEYBLADE_SO_06", &CS_MODELS, "w_so080", Some(Vec3::new(250.0, 0.0, 0.0)), name)?;
    let pack = kh_host::audit_a::project().join("build").join("sora_pack");
    s.fx.load_xsi_root(&pack).expect("the XS effect root");
    if ceiling {
        s.rig.world = kh_core::test_world::TestWorld::floor().with_box(Vec3::new(-5000.0, -5000.0, 500.0), Vec3::new(5000.0, 5000.0, 560.0));
    }
    let mut r = CeilingRig { s, cams: kh_camera::form_bp::FormBpCameras::new(), run: CeilingRun::default() };
    r.idle(5);
    for cmd in [0x90, 0x85] {
        r.s.rig.sora.debug_add_style(100.0);
        r.idle(20);
        assert!(r.s.rig.sora.combat().situation.has(cmd), "{cmd:#x} offered");
        r.step(&[PadButton::SITUATION]);
        let mut done = false;
        for _ in 0..600 {
            if r.step(&[]) != StateId::Stylechange && r.s.rig.sora.combat().form.cmd == cmd {
                done = true;
                break;
            }
        }
        assert!(done, "STYLECHANGE to {cmd:#x}");
        r.idle(30);
    }
    for _ in 0..30 {
        if r.s.rig.sora.combat().situation.slot_b.map(|e| e.kind) == Some(0x9F) {
            break;
        }
        r.s.rig.sora.debug_add_style(100.0);
        r.idle(10);
    }
    assert_eq!(r.s.rig.sora.combat().situation.slot_b.map(|e| e.kind), Some(0x9F), "FINISH_SK on offer");
    r.step(&[PadButton::SITUATION]);
    for _ in 0..1500 {
        if r.step(&[]) != StateId::Stylefinish && r.s.rig.sora.combat().form.cmd == 0 {
            break;
        }
    }
    r.idle(60);
    assert!(r.s.rows.iter().any(|x| x.clip == "7423" && x.state == StateId::Stylefinish), "FINISH_SK's 7423 played");
    let run = r.run;
    let mut text = String::new();
    for (k, b, p) in &run.bp {
        text.push_str(&format!("step {k:5} 7423 BpEvent {} param {p}\n", if *b { "start" } else { "end" }));
    }
    for (k, on) in &run.tower {
        text.push_str(&format!("step {k:5} camera stack: tower {}\n", if *on { "ON" } else { "OFF" }));
    }
    for (k, h, at, sora, drawn) in &run.spawns {
        text.push_str(&format!("step {k:5} spawn {} handle {h} {at} (his actor location {sora}) drawn {drawn}\n", kh_core::states::bp_event::CEILING_PARTICLE));
    }
    for (k, h, imm) in &run.stops {
        text.push_str(&format!("step {k:5} stop handle {h} immediate {imm}\n"));
    }
    for l in &run.log {
        text.push_str(l);
        text.push('\n');
    }
    let dir = kh_host::audit_a::project().join("build").join("sora_tmp").join("weapons").join("AUDIT-B").join("W-FIN").join("traces");
    std::fs::create_dir_all(&dir).ok();
    std::fs::write(dir.join(format!("{name}.trace")), text).ok();
    Some(run)
}

/// UNDER A CEILING (fz_key's 7419 trace hits): at 7423's Param-1 start the look begins in the same step on both seams (the
/// core spawns vw_so073_fin_fsg0 at his actor location, rotation 0, persistent, and the effect host draws it; the camera
/// stack's `tower` edge ON for the renderer's hide / lights / post-process); at 7423's end both end in the same step (the
/// particle stopped at once, FadeOut(0); `tower` OFF). Once each.
#[test]
fn ceiling_look_under_a_ceiling_switches_on_and_off_with_7423() {
    let Some(r) = finish_sk_run(true, "auditb_fin_ceiling") else { return };
    let start = r.bp.iter().find(|(_, b, p)| *b && *p != 0).map(|x| x.0).expect("7423's Param-1 window start");
    let end = r.bp.iter().find(|(_, b, _)| !*b).map(|x| x.0).expect("7423's window end");
    assert_eq!(r.spawns.len(), 1, "one particle: {:?}", r.spawns);
    let (k, h, at, sora, drawn) = &r.spawns[0];
    assert_eq!(*k, start, "spawned at the Param-1 start");
    let p: Vec<f32> = at.trim_start_matches("World(Vec3(").trim_end_matches("))").split(',').filter_map(|x| x.trim().parse().ok()).collect();
    assert!(at.starts_with("World(") && p.len() == 3 && (Vec3::new(p[0], p[1], p[2]) - *sora).length() < 1.0, "in the world at his actor location: {at} vs {sora}");
    assert!(*drawn > 0, "the effect host draws it (the other-keyblade rule lets Crystal Snow's FINISH_SK spawn Ever After's vw_so073)");
    assert_eq!(r.stops, vec![(end, *h, true)], "stopped at once at the 7423 end");
    assert_eq!(r.tower, vec![(start, true), (end, false)], "the renderer's look on / off in the same steps");
}

/// NO CEILING: no particle, no look.
#[test]
fn ceiling_look_without_a_ceiling_does_nothing() {
    let Some(r) = finish_sk_run(false, "auditb_fin_no_ceiling") else { return };
    assert!(r.bp.iter().any(|(_, b, p)| *b && *p != 0), "7423's Param-1 window was sent");
    assert!(r.spawns.is_empty() && r.stops.is_empty() && r.tower.is_empty(), "nothing: {:?} {:?} {:?}", r.spawns, r.stops, r.tower);
}

// ================================================================================================ (AUDIT-B W-FIN) items 1, 2, 4
//
// `timing_*`: FORM-WEAPON / PAWN ROWS CARRY FadeIn / FadeOut / StartDelayTime. KH3's attach post-spawn 0x141772CD0 gives a row's
// FadeIn to the component's FadeIn(t) and its StartDelayTime to +0xB88; the FADEOUT end type fades over FadeOut (0x141763E60).
// The seam: kh-core `Effects::timing` (EffectTiming) right after the spawn, sent by every form-weapon / pawn row player
// (claws / nano `wfx`, ship.rs `wnotify_pass` for ship / hammer / arrowgun / shield / staff); kh-host `EffectCommand::Timing`,
// applied by `EffectHost` to the spawn's placement. The only AUDIT-B rows with a FadeIn / StartDelayTime: Crystal Snow's
// transform unit w_so081 7401 (StyleChange7): vw_so086_cng_stm00 StartDelayTime 0.5, FADEOUT 0.2 (census of every
// keyblades/*/model.json clip notify: SECTION.md). Those rows did not exist before this item (the claws generator skipped the
// transform units w_so061 / w_so081): added, so their clips play too.
// `chandelier_*`: the shatter pieces placed as the data says (item 1) and the Chandelier's own root shape (item 4).

/// One effect spawn of the step rows of a clip: (step, clip time, name, timing, drawn, handle).
type Spawned = (u64, f32, String, Option<kh_core::host::EffectTiming>, usize, u32);

fn spawned_in(s: &Session, seq: &str) -> Vec<Spawned> {
    s.rows.iter().filter(|r| r.clip == seq).flat_map(|r| r.effects.iter().filter(|f| f.stop.is_none()).map(move |f| (r.step, r.time, f.name.clone(), f.timing, f.drawn, f.handle))).collect()
}

/// Crystal Snow into Blizzard Claws (I401 StyleChange18) and Blizzard Blades (7401 StyleChange7).
fn crystal_snow_forms(name: &str) -> Option<Session> {
    let mut s = Session::new("WEP_KEYBLADE_SO_06", &CS_MODELS, "w_so080", Some(Vec3::new(250.0, 0.0, 0.0)), name)?;
    let pack = kh_host::audit_a::project().join("build").join("sora_pack");
    s.fx.load_xsi_root(&pack).expect("the XS effect root");
    s.idle(5);
    for cmd in [0x90, 0x85] {
        s.rig.sora.debug_add_style(100.0);
        s.idle(20);
        assert!(s.rig.sora.combat().situation.has(cmd), "{cmd:#x} offered");
        s.step(&[PadButton::SITUATION]);
        let mut done = false;
        for _ in 0..600 {
            if s.step(&[]) != StateId::Stylechange && s.rig.sora.combat().form.cmd == cmd {
                done = true;
                break;
            }
        }
        assert!(done, "STYLECHANGE to {cmd:#x}");
        s.idle(60);
    }
    Some(s)
}

fn write_trace(name: &str, text: &str) {
    let dir = kh_host::audit_a::project().join("build/sora_tmp/weapons/AUDIT-B/W-FIN/traces");
    std::fs::create_dir_all(&dir).ok();
    std::fs::write(dir.join(name), text).ok();
}

/// THE TRANSFORM UNIT'S OWN CLIPS PLAY, WITH THEIR TIMING: w_so081's I401 and 7401 rows (`wfx.CL2.w_so081.I401.*`,
/// `wfx.SK.w_so081.7401.*`) start in their windows and are drawn; vw_so086_cng_stm00 carries StartDelayTime 0.5 and FADEOUT 0.2
/// to the host; the LOOPEND rows (FadeOut 0.2 in the data, unused by LOOPEND) carry none. The host holds such a spawn back 0.5 s
/// (its system lives with no particle) and then draws it.
#[test]
fn timing_of_the_transform_unit_rows_reaches_the_host() {
    use kh_core::combat::forms::claws;
    let Some(mut s) = crystal_snow_forms("auditb_fin_timing") else { return };
    let mut text = String::new();
    for (seq, cmd) in [("I401", 0x90), ("7401", 0x85)] {
        let want: Vec<&str> = claws::weapon_fx_rows(cmd).filter(|(_, r)| r.clip == seq && r.flags.get(8) == Some(&"w_so081") && r.flags.get(1) == Some(&"effect")).map(|(_, r)| r.logical).collect();
        let got = spawned_in(&s, seq);
        for (step, t, name, timing, drawn, h) in &got {
            text.push_str(&format!("{seq} step {step:5} t {t:.3} {name} handle {h} drawn {drawn} timing {timing:?}\n"));
        }
        assert!(want.len() >= 6, "{seq}: the unit's effect rows {want:?}");
        for w in &want {
            assert!(got.iter().any(|g| g.2 == *w), "{seq}: {w} spawned ({got:?})");
        }
        let sounds: Vec<String> = s.rows.iter().filter(|r| r.clip == seq).flat_map(|r| r.sounds.iter().map(|x| x.name.clone())).collect();
        for x in &sounds {
            text.push_str(&format!("{seq} sound {x}\n"));
        }
        let unit_sound = claws::weapon_fx_rows(cmd).find(|(_, r)| r.clip == seq && r.flags.get(8) == Some(&"w_so081") && r.flags.get(1) == Some(&"sound")).map(|(_, r)| r.logical).expect("the unit's AttachSound row");
        assert!(sounds.iter().any(|n| n == unit_sound), "{seq}: the unit's sound {unit_sound} ({sounds:?})");
    }
    let got = spawned_in(&s, "7401");
    let (_, _, _, timing, drawn, _) = got.iter().find(|g| g.2 == "vw_so086_cng_stm00").cloned().expect("stm00");
    let t = timing.expect("stm00 carries its timing");
    assert!((t.start_delay - 0.5).abs() < 1e-4 && (t.fade_out - 0.2).abs() < 1e-4 && t.fade_in == 0.0, "{t:?}");
    assert!(drawn > 0, "the host draws stm00");
    for (_, _, name, timing, ..) in &got {
        if name.starts_with("vw_so086_cng_tub") {
            assert!(timing.is_none(), "LOOPEND rows carry no fade: {name} {timing:?}");
        }
    }
    // the host's side: a spawn with that timing is held back 0.5 s (the system lives, no particle), then drawn
    let particles = |s: &Session| s.fx.world.live().iter().filter(|l| l.name.eq_ignore_ascii_case("vw_so086_cng_stm00")).map(|l| l.emitters.iter().map(|e| e.3).sum::<usize>()).sum::<usize>();
    s.idle(200);
    assert_eq!(particles(&s), 0, "the session's own stm00 has ended");
    let spawn = kh_core::EffectSpawn { id: kh_core::EffectId(1), name: "vw_so086_cng_stm00", at: kh_core::Attach::World(Vec3::new(0.0, 0.0, 100.0)), rotation: Vec3::ZERO, scale: 1.0, persistent: true };
    let out = s.rig.out.clone();
    let target = |_: kh_core::TargetId| None;
    let tap = kh_host::audio::PoseTap::default();
    let scene = kh_host::vfx::Scene { out: &out, sora: &s.rig.sora, tap: &tap, target: &target, ground: Some(0.0), eye: Vec3::new(-500.0, 0.0, 200.0) };
    let handle = kh_core::EffectHandle(0x7D00_0001);
    s.fx.step(1.0 / 60.0, &[kh_host::seam::EffectCommand::Spawn(handle, spawn), kh_host::seam::EffectCommand::Timing(handle, t)], &scene);
    let (mut held, mut first) = (0, None);
    for k in 1..60 {
        let n = particles(&s);
        text.push_str(&format!("fresh stm00 (StartDelayTime {:.2}) step {k:2} particles {n}\n", t.start_delay));
        if k < 29 {
            held += usize::from(n == 0);
        }
        if n > 0 && first.is_none() {
            first = Some(k);
        }
        s.fx.step(1.0 / 60.0, &[], &scene);
    }
    assert_eq!(held, 28, "no particle during the 0.5 s delay");
    assert!(first.is_some_and(|k| k <= 40), "drawn after the delay: {first:?}");
    write_trace("auditb_fin_timing.trace", &text);
}

/// THE SHATTER PIECES AS THE DATA PLACES THEM (item 1): the Chandelier's rows carry the data's rotation in ITS mesh frame (yaw
/// only: no roll), and the fragment pieces of hahen00 .. 03 spawned in that frame are w_so088's own parts (their Offsets lie on
/// its mesh, 2.5 .. 4.3 m over the root) 3 m lower (the parent Control_lp's EmitterOrigin z -300): at the floor line where the
/// falling body crosses it, inside its 1.85 m radius, unturned by the emitter's roll 90 (not lying beside it).
#[test]
fn chandelier_shatter_pieces_stand_where_its_mesh_is() {
    use kh_core::combat::forms::claws;
    let pack = kh_host::audit_a::project().join("build").join("sora_pack");
    let mut text = String::new();
    let mut n = 0;
    for (_, r) in claws::pawn_fx_rows().filter(|(_, r)| r.logical.contains("_hahen")) {
        n += 1;
        for t in [0.0, 4.9, 5.6] {
            let (at, rot) = claws::wfx_world(r, t, Vec3::new(100.0, 50.0, 0.0), 30.0);
            text.push_str(&format!("{} t {t:.1}: at {at} rotation {rot}\n", r.logical));
            assert!((at - Vec3::new(100.0, 50.0, 0.0)).length() < 0.1, "socket None = the component (the root): {at}");
            assert!((rot - Vec3::new(0.0, 30.0, 0.0)).length() < 0.1, "the data's rotation, yaw only: {rot}");
        }
    }
    assert_eq!(n, 4, "hahen00 .. 03");
    for k in 0..4 {
        let name = format!("vw_so088_fin_hahen0{k}");
        // the XS root's own file first (effects_xwd/: hahen02 / 03 with their pieces' meshes, as kh-host's XS root reads them)
        let read = |dir: &str| std::fs::read_to_string(pack.join(format!("xsi/vfx/{dir}/{name}.json")));
        let Ok(json) = read("effects_xwd").or_else(|_| read("effects")) else {
            eprintln!("no {name}: skipped");
            return;
        };
        let mut world = kh_vfx::World::new();
        world.register(kh_vfx::EffectDef::parse(&json).expect("the effect"));
        world.spawn(&name, kh_vfx::Xform::at(Vec3::ZERO), kh_vfx::SpawnOptions::default()).expect("spawned");
        world.step(1.0 / 60.0);
        let mut list = kh_vfx::DrawList::default();
        world.draw(1.0, &kh_vfx::Camera::DEFAULT, &mut list);
        let pieces: Vec<Vec3> = list.meshes.iter().filter(|m| world.mesh_name(m.mesh).is_some_and(|x| x.starts_with("kmd_so088"))).map(|m| Vec3::from(m.transform.translation)).collect();
        assert!(!pieces.is_empty(), "{name}: pieces drawn");
        let (lo, hi) = pieces.iter().fold((f32::MAX, f32::MIN), |(a, b), p| (a.min(p.z), b.max(p.z)));
        let reach = pieces.iter().map(|p| p.truncate().length()).fold(0.0, f32::max);
        text.push_str(&format!("{name}: {} pieces, height {lo:.0} .. {hi:.0} cm, farthest {reach:.0} cm from the axis\n", pieces.len()));
        // the pieces' Offsets (authored on w_so088's own mesh: z 249 .. 432 over its root) under the parent Control_lp's
        // EmitterOrigin (0, 0, -300), unturned: the chandelier's parts where its falling body crosses the floor
        assert!(lo > -100.0 && hi < 150.0, "{name}: at the floor line under the root (z {lo} .. {hi})");
        assert!(reach < 200.0, "{name}: inside the chandelier's 1.85 m radius ({reach})");
    }
    write_trace("auditb_fin_hahen.trace", &text);
}

/// THE CHANDELIER'S OWN ROOT SHAPE (item 4): StylePawn/w_so088's TresRoot is a SPHERE r 250 at z 250 (the pawn row's values [5],
/// [6]); FINISH_SK's chandelier stands on the floor under him and its centre is that root + 250, not Sora's capsule centre.
#[test]
fn chandelier_centre_is_its_own_root_shape() {
    use kh_core::combat::forms::claws;
    let row = claws::FORM_CLAWS.iter().find(|r| r.id == "pawn.SK.w_so088").expect("pawn.SK.w_so088");
    assert_eq!(&row.values[5..7], &[250.0, 250.0], "TresRoot SPHERE: centre z 250, radius 250");
    let Some(mut s) = crystal_snow_forms("auditb_fin_centre") else { return };
    for _ in 0..30 {
        if s.rig.sora.combat().situation.slot_b.map(|e| e.kind) == Some(0x9F) {
            break;
        }
        s.rig.sora.debug_add_style(100.0);
        s.idle(10);
    }
    s.step(&[PadButton::SITUATION]);
    let mut seen = None;
    for _ in 0..400 {
        s.step(&[]);
        if let Some(c) = s.rig.sora.combat().xs.chandelier {
            seen = Some(c);
            break;
        }
    }
    let c = seen.expect("FINISH_SK's chandelier");
    write_trace("auditb_fin_centre.trace", &format!("FINISH_SK chandelier: ground {} centre {} (centre - ground {})\n", c.ground, c.center, c.center - c.ground));
    assert!((c.center - c.ground - Vec3::Z * 250.0).length() < 1e-3, "centre {} over ground {}", c.center, c.ground);
    assert!(c.ground.z.abs() < 1.0, "its root on the floor: {}", c.ground);
}
