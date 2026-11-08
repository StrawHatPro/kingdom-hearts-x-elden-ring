//! (AUDIT-B F-NANO) NANO ARMS END TO END, LOOK AND SOUND: kh-core (Nano Gear, CHANGE_GM 0x86) -> the effect / audio seams ->
//! `kh_host::vfx::EffectHost` (main + keyblade roots of build/sora_pack) and `kh_host::audio::HostAudio` (the audio pack's
//! routes), recorded by `kh_host::audit_a::Session`, plus the port's camera stack's form layer (`kh_camera::forms::select` +
//! `FormCameras`, as kh-er cam.rs). One session: the offer, the transformation (StyleChange8 8401), the combo near a dummy
//! (830x, the crash / stamp / yo-yos, the Rocket Fist 8415 and its flying fist), the hookshot (8423 -> 8424), the Nano Counter
//! (MagicCast 1341), the offensive dodge (8051), a landing, FINISH_GM (8416 / 8417 / 8418 with the gate w_so174), the end.
//! Asserts: every sound / voice / effect notify of Sora's clips fired is emitted and resolved (audit_a::coverage); every
//! effect / sound of the weapon's and the gate's own clips (sheet rows `wfx.*`) of the clips played is emitted and found;
//! the fist's ead_so170_com0 group 1620 effects ride with it; the gate's hits spawn vw_so174_hit00; the blueprint cameras
//! (Gimmick-key_Change during 8401, Gimmick-key_Finish_cam from 8416 to the finish's end). Logs: build/sora_tmp/weapons/
//! AUDIT-B/F-NANO/traces/. Skipped, saying so, without the pack.

use std::fmt::Write as _;

use glam::{Vec2, Vec3};
use kh_camera::forms::{select, FormCameras};
use kh_camera::{CameraTarget, CameraView, Rot, TargetState};
use kh_core::enums::ETresDamageKind;
use kh_core::states::HitOnPlayer;
use kh_core::{PadButton, StateId, TargetId};
use kh_host::audit_a::{self, Session};

fn traces() -> std::path::PathBuf {
    let d = audit_a::project().join("build").join("sora_tmp").join("weapons").join("AUDIT-B").join("F-NANO").join("traces");
    std::fs::create_dir_all(&d).ok();
    d
}

/// The camera layer of each step (kh-er cam.rs: the form cameras over the field camera).
struct Cams {
    forms: FormCameras,
    csv: String,
    /// (step, state, clip, which camera)
    shown: Vec<(u64, StateId, &'static str, Option<&'static str>)>,
}

impl Cams {
    fn step(&mut self, s: &Session) {
        let out = &s.rig.out;
        let target = CameraTarget { position: out.position, yaw: out.yaw, velocity: Vec3::ZERO, state: TargetState::Idle, collision_half_height: 89.0 };
        let (sin, cos) = out.yaw.to_radians().sin_cos();
        let behind = out.position + Vec3::new(-400.0 * cos, -400.0 * sin, 200.0);
        let rot = Rot::new(-15.0, out.yaw, 0.0);
        let under = CameraView { position: behind, rotation: rot, rotation_target: rot, look_point: out.position + Vec3::Z * 100.0, distance: 400.0, auto_power: 0.0, stick_wait: 0.0, still_wait: 0.0, reset_wait: 0.0, slope_pitch: 0.0, stick_active: false, followed: false, collided: false };
        let sel = select(&s.rig.sora, false, &|_| s.rig.dummy.map(|d| d.info.center));
        let v = self.forms.step(1.0 / 60.0, sel.shot, &target, &under, Vec2::ZERO, &s.rig.world);
        let which = self.forms.nano.running().map(|r| r.0).or(v.map(|_| "blend"));
        let clip = out.anim_ticked.body_samples()[0].0;
        let (p, r, fov) = v.map_or((under.position, under.rotation, 100.0), |v| (v.view.position, v.view.rotation, v.fov));
        let rel = p - out.position;
        let _ = writeln!(self.csv, "{},{:?},{},{:.3},{:.1},{:.1},{:.1},{:.1},{:.1},{:.1},{:.1},{}", out.step, out.state, clip, out.anim_ticked.time, rel.x, rel.y, rel.z, r.pitch, r.yaw, r.roll, fov, which.unwrap_or("field"));
        self.shown.push((out.step, out.state, clip, which));
    }
}

/// Every `wfx` row (the weapon's / the gate's / the fist's own effects and sounds) of the clips played: emitted on a step of
/// its clip within two steps of its begin (an effect also drawn), one line each. (text, the missing ones)
fn clip_fx(rows: &[audit_a::StepRow], played: &[&str], fist_seen: bool) -> (String, Vec<String>) {
    let mut text = String::from("# the weapon's (w_so172) / the gate's (w_so174) / the fist's (w_so173) own effects and sounds: row, owner, kind, name, socket, begin -> the step that emitted it\n");
    let mut missing = Vec::new();
    for r in kh_core::combat::forms::nano::FORM_NANO.iter().filter(|r| r.what == "wfx") {
        let owner = r.flags[0];
        if !(if owner == "fist" { fist_seen } else { played.contains(&r.clip) }) {
            continue;
        }
        let begin: f32 = r.windows.first().and_then(|w| w.split('-').next()).and_then(|a| a.parse().ok()).unwrap_or(0.0);
        let end: f32 = r.windows.first().and_then(|w| w.split('-').nth(1)).and_then(|a| a.parse().ok()).unwrap_or(begin);
        let name = r.logical.rsplit(['/', '.']).next().unwrap_or(r.logical).to_lowercase();
        // at its begin (two steps' slack); a STATE one also when its clip starts inside its window (UE's overlap rule)
        let state = r.flags[2] == "state";
        let near = |row: &audit_a::StepRow| owner == "fist" || (row.clip == r.clip && ((row.time - begin).abs() <= 2.5 / 60.0 + 1.0e-3 || (state && row.time >= begin && row.time < end)));
        let hit = rows.iter().find(|row| {
            near(row)
                && if r.flags[1] == "sound" {
                    row.sounds.iter().any(|x| x.stop.is_none() && x.name.to_lowercase().ends_with(&name))
                } else {
                    row.effects.iter().any(|e| e.stop.is_none() && e.name == name && e.drawn > 0)
                }
        });
        let _ = writeln!(text, "{} {} {} {} {} socket {} begin {begin:.4} -> {}", if hit.is_some() { "PASS" } else { "FAIL" }, r.id, owner, r.flags[1], name, r.flags[7], hit.map_or("not emitted / not drawn at its time".to_string(), |h| format!("step {} {} @{:.3}", h.step, h.clip, h.time)));
        if hit.is_none() {
            missing.push(format!("{} {} @{begin:.4}", r.id, name));
        }
    }
    (text, missing)
}

fn run(s: &mut Session, c: &mut Cams, held: &[PadButton]) -> StateId {
    let st = s.step(held);
    c.step(s);
    st
}

fn idle(s: &mut Session, c: &mut Cams, n: usize) {
    for _ in 0..n {
        run(s, c, &[]);
    }
}

#[test]
fn nano_arms_looks_and_sounds_like_kh3() {
    let Some(mut s) = Session::new("WEP_KEYBLADE_SO_08", &["w_so170", "w_so172", "w_so173", "w_so174"], "w_so170", Some(Vec3::new(200.0, 0.0, 0.0)), "audit_b_nano") else { return };
    let mut c = Cams { forms: FormCameras::default(), csv: "step,state,clip,time,cam_dx,cam_dy,cam_dz,pitch,yaw,roll,fov,camera\n".to_string(), shown: Vec::new() };
    idle(&mut s, &mut c, 5);
    // ---- the offer -> STYLECHANGE (StyleChange8 = 8401)
    s.rig.sora.debug_add_style(100.0);
    idle(&mut s, &mut c, 15);
    run(&mut s, &mut c, &[PadButton::SITUATION]);
    assert!(s.until(600, |s| s.rig.out.state == StateId::Stylechange), "STYLECHANGE");
    for _ in 0..600 {
        if run(&mut s, &mut c, &[]) != StateId::Stylechange {
            break;
        }
    }
    assert_eq!(s.rig.sora.combat().form.cmd, 0x86, "Nano Arms");
    // ---- the combo near the dummy
    idle(&mut s, &mut c, 30);
    for k in 0..900 {
        let st = if k % 2 == 0 { run(&mut s, &mut c, &[PadButton::ATTACK]) } else { run(&mut s, &mut c, &[]) };
        if k > 30 && matches!(st, StateId::Idle | StateId::Run | StateId::Jump) {
            break;
        }
    }
    // the flying fist's flight
    let mut fist_seen = false;
    for _ in 0..150 {
        run(&mut s, &mut c, &[]);
        fist_seen |= s.rig.sora.combat().nano.rockets.iter().any(|r| r.stage == 1);
    }
    // ---- far: the hookshot
    let x = s.rig.out.position.x;
    if let Some(d) = s.rig.dummy.as_mut() {
        d.info.position = Vec3::new(x + 700.0, 0.0, 0.0);
        d.info.center = d.info.position + Vec3::Z * 90.0;
    }
    idle(&mut s, &mut c, 20);
    run(&mut s, &mut c, &[PadButton::ATTACK]);
    idle(&mut s, &mut c, 150);
    // ---- the Nano Counter: a guarded hit, Attack in the window
    let out = s.rig.out.clone();
    let fwd = Vec3::new(out.yaw.to_radians().cos(), out.yaw.to_radians().sin(), 0.0);
    let at = Vec3::new(out.position.x, out.position.y, 0.0) + fwd * 200.0;
    if let Some(d) = s.rig.dummy.as_mut() {
        d.info.position = at;
        d.info.center = at + Vec3::Z * 90.0;
    }
    idle(&mut s, &mut c, 20);
    run(&mut s, &mut c, &[PadButton::GUARD]);
    idle(&mut s, &mut c, 10);
    let d = (Vec3::new(out.position.x, out.position.y, 0.0) - at).normalize_or_zero();
    s.rig.sora.take_hit(HitOnPlayer { direction: d, causer_yaw: d.y.atan2(d.x).to_degrees(), ..HitOnPlayer::new(ETresDamageKind::Small, 30.0, TargetId(1), at) });
    idle(&mut s, &mut c, 1);
    run(&mut s, &mut c, &[PadButton::ATTACK]);
    idle(&mut s, &mut c, 90);
    // ---- the offensive dodge (8051) and a landing
    idle(&mut s, &mut c, 30);
    s.stick = Vec2::new(0.0, 1.0);
    run(&mut s, &mut c, &[PadButton::GUARD]);
    idle(&mut s, &mut c, 4);
    s.stick = Vec2::ZERO;
    idle(&mut s, &mut c, 60);
    run(&mut s, &mut c, &[PadButton::JUMP]);
    idle(&mut s, &mut c, 90);
    // ---- FINISH_GM
    idle(&mut s, &mut c, 30);
    let finish_from = s.rows.len();
    let start = s.rig.out.position;
    run(&mut s, &mut c, &[PadButton::SITUATION]);
    let mut gate_hits = 0;
    for _ in 0..600 {
        let st = run(&mut s, &mut c, &[]);
        gate_hits += s.rows.last().map_or(0, |r| r.effects.iter().filter(|e| e.name == "vw_so174_hit00").count());
        if st != StateId::Stylefinish && s.rig.sora.combat().form.cmd == 0 {
            break;
        }
    }
    idle(&mut s, &mut c, 120);
    s.finish();
    let rows = &s.rows;

    // ================================================================ the logs
    let dir = traces();
    let report = audit_a::report(rows);
    std::fs::write(dir.join("nano_session.report.txt"), &report).ok();
    std::fs::write(dir.join("finish_camera.csv"), &c.csv).ok();
    let mut sounds = String::from("# every SE / voice the seam carried, per step, with the audio side's answer\n");
    for r in rows {
        for x in r.sounds.iter().filter(|x| x.stop.is_none()) {
            let short = x.name.rsplit(['/', '.']).next().unwrap_or(&x.name).to_lowercase();
            let lines: Vec<String> = r.audio.iter().filter(|a| a.1.to_lowercase().ends_with(&short)).map(|a| format!("{} {} {}", a.0, a.2, a.3).trim().to_string()).collect();
            let _ = writeln!(sounds, "step {:5} {:?} {} @{:.3} {} -> {}", r.step, r.state, r.clip, r.time, x.name, if lines.is_empty() { "-".to_string() } else { lines.join(" | ") });
        }
    }
    std::fs::write(dir.join("nano_sounds.txt"), &sounds).ok();
    if let Ok(csv) = std::fs::read(audit_a::path_in_logs("audit_b_nano.audio.csv")) {
        std::fs::write(dir.join("nano_session.audio.csv"), csv).ok();
    }

    // ================================================================ Sora's clips: every notify emitted and resolved
    let cov = audit_a::coverage(rows);
    let failed: Vec<String> = cov.iter().filter(|c| !c.ok && !trail_ok(c)).map(|c| format!("{} @{:.3} {} {} -> {}", c.clip, c.time, c.want.kind(), c.want.seam_name(), c.resolved)).collect();
    let played: Vec<&str> = audit_a::clips(rows).iter().map(|c| c.0).collect();
    for clip in ["8401", "8301", "8302", "8303", "8411", "8415", "8423", "8424", "1341", "8051", "8416", "8417", "8418"] {
        assert!(played.contains(&clip), "{clip} played: {played:?}");
    }
    // ================================================================ the weapon's and the gate's own clips (wfx rows)
    let spawned: Vec<(u64, String, usize)> = rows.iter().flat_map(|r| r.effects.iter().filter(|e| e.stop.is_none()).map(move |e| (r.step, e.name.clone(), e.drawn))).collect();
    let (mut wfx, missing) = clip_fx(rows, &played, fist_seen);
    let _ = writeln!(wfx, "# gate hit effects vw_so174_hit00: {gate_hits}");
    std::fs::write(dir.join("nano_clip_fx.txt"), &wfx).ok();
    // ================================================================ the cameras
    let change = c.shown.iter().filter(|x| x.2 == "8401" && x.3 == Some(kh_camera::forms_nano::CHANGE_ANIM)).count();
    let finish: Vec<_> = c.shown.iter().filter(|x| x.1 == StateId::Stylefinish).collect();
    let finish_cam = finish.iter().filter(|x| x.3 == Some(kh_camera::forms_nano::FINISH_ANIM)).count();
    let combo_cam = c.shown.iter().filter(|x| x.3 == Some(kh_camera::forms_nano::COMBO_FINISH_ANIM)).count();
    let summary = format!("coverage {} notifies, {} ok; wfx missing {}; gate hits {gate_hits}; cameras: change {change} steps, finish {finish_cam} of {} STYLEFINISH steps, combo finish {combo_cam}; log {:?}", cov.len(), cov.len() - failed.len(), missing.len(), finish.len(), c.forms.nano.log);
    std::fs::write(dir.join("nano_summary.txt"), format!("{summary}\nFAILED notifies:\n{}\nMISSING clip fx:\n{}\n", failed.join("\n"), missing.join("\n"))).ok();
    eprintln!("{summary}");
    assert!(failed.is_empty(), "notifies not emitted / resolved: {failed:#?}");
    assert!(missing.is_empty(), "the weapon's / gate's clip effects missing: {missing:#?}");
    assert!(fist_seen && spawned.iter().any(|(_, n, d)| n == "vw_so173_blc00" && *d > 0), "the flying fist's group 1620 effects");
    assert!(gate_hits > 0, "the gate's hit effect vw_so174_hit00");
    assert!(change > 200, "Gimmick-key_Change runs through 8401: {change}");
    assert!(finish_cam + 6 >= finish.len() && finish_cam > 280, "Gimmick-key_Finish_cam from 8416 to the end: {finish_cam} / {}", finish.len());
    // Sora stays at the gate (attached to its root: the gate has no X_link01_skt; Flying, no root motion in 8416 .. 8418)
    let drift = rows[finish_from..].iter().filter(|r| r.state == StateId::Stylefinish).map(|r| (r.position - start).length()).fold(0.0f32, f32::max);
    assert!(drift < 5.0, "Sora held at the gate during FINISH_GM: {drift} cm");
}

fn place_dummy(s: &mut Session, at: Vec3, centre_z: f32, airborne: bool) {
    if let Some(d) = s.rig.dummy.as_mut() {
        d.info.position = at;
        d.info.center = at + Vec3::Z * centre_z;
        d.info.flags = kh_core::TargetFlags(kh_core::TargetFlags::ALIVE | kh_core::TargetFlags::HITTABLE | kh_core::TargetFlags::LOCKABLE | kh_core::TargetFlags::ENEMY | if airborne { kh_core::TargetFlags::AIRBORNE } else { 0 });
    }
}

fn ahead(s: &Session, d: f32, z: f32) -> Vec3 {
    let o = &s.rig.out;
    Vec3::new(o.position.x + o.yaw.to_radians().cos() * d, o.position.y + o.yaw.to_radians().sin() * d, z)
}

/// (AUDIT-B F-NANO) The clips the first session does not reach: the giant stamp 8413 (a low lock point), the rise 8419 (a
/// target above), the air combo's drill 8412, the fall 8420 (a target below him in the air), the air dodge 8054 and the
/// landings: every effect / sound of Sora's notifies and of the weapon's own clips.
#[test]
fn the_other_nano_clips_look_and_sound_like_kh3() {
    let Some(mut s) = Session::new("WEP_KEYBLADE_SO_08", &["w_so170", "w_so172", "w_so173", "w_so174"], "w_so170", Some(Vec3::new(200.0, 0.0, 0.0)), "audit_b_nano_2") else { return };
    let mut c = Cams { forms: FormCameras::default(), csv: String::new(), shown: Vec::new() };
    idle(&mut s, &mut c, 5);
    s.rig.sora.debug_add_style(100.0);
    idle(&mut s, &mut c, 15);
    run(&mut s, &mut c, &[PadButton::SITUATION]);
    s.until(600, |s| s.rig.out.state == StateId::Stylechange);
    s.until(600, |s| s.rig.out.state != StateId::Stylechange);
    idle(&mut s, &mut c, 30);
    let combo = |s: &mut Session, c: &mut Cams, n: usize| {
        for k in 0..n {
            if k % 2 == 0 { run(s, c, &[PadButton::ATTACK]) } else { run(s, c, &[]) };
        }
    };
    // a low lock point: the crash stays on the ground, the second finisher is the giant stamp 0xB7
    let at = ahead(&s, 200.0, 0.0);
    place_dummy(&mut s, at, 10.0, false);
    combo(&mut s, &mut c, 400);
    idle(&mut s, &mut c, 120);
    // a target above him: the rise 0xBA
    let at = ahead(&s, 150.0, 300.0);
    place_dummy(&mut s, at, 90.0, true);
    idle(&mut s, &mut c, 10);
    combo(&mut s, &mut c, 6);
    idle(&mut s, &mut c, 90);
    // the air combo level with an airborne target: the drill 0xB6, the yo-yos 0xB8
    let at = ahead(&s, 200.0, 300.0);
    place_dummy(&mut s, at, 90.0, true);
    run(&mut s, &mut c, &[PadButton::JUMP]);
    for _ in 0..12 {
        run(&mut s, &mut c, &[PadButton::JUMP]);
    }
    combo(&mut s, &mut c, 300);
    // in the air above a target on the floor: the fall 0xBB
    let at = ahead(&s, 150.0, 0.0);
    place_dummy(&mut s, at, 90.0, false);
    run(&mut s, &mut c, &[PadButton::JUMP]);
    for _ in 0..20 {
        run(&mut s, &mut c, &[PadButton::JUMP]);
    }
    combo(&mut s, &mut c, 4);
    idle(&mut s, &mut c, 120);
    // the air dodge (8054) and its landing
    run(&mut s, &mut c, &[PadButton::JUMP]);
    for _ in 0..15 {
        run(&mut s, &mut c, &[PadButton::JUMP]);
    }
    s.stick = Vec2::new(0.0, 1.0);
    run(&mut s, &mut c, &[PadButton::GUARD]);
    idle(&mut s, &mut c, 4);
    s.stick = Vec2::ZERO;
    idle(&mut s, &mut c, 120);
    s.finish();
    let rows = &s.rows;
    let dir = traces();
    std::fs::write(dir.join("nano_session2.report.txt"), audit_a::report(rows)).ok();
    if let Ok(csv) = std::fs::read(audit_a::path_in_logs("audit_b_nano_2.audio.csv")) {
        std::fs::write(dir.join("nano_session2.audio.csv"), csv).ok();
    }
    let played: Vec<&str> = audit_a::clips(rows).iter().map(|c| c.0).collect();
    let cov = audit_a::coverage(rows);
    let failed: Vec<String> = cov.iter().filter(|c| !c.ok && !trail_ok(c)).map(|c| format!("{} @{:.3} {} {} -> {}", c.clip, c.time, c.want.kind(), c.want.seam_name(), c.resolved)).collect();
    let (text, missing) = clip_fx(rows, &played, false);
    std::fs::write(dir.join("nano_clip_fx2.txt"), &text).ok();
    let summary = format!("clips {played:?}\ncoverage {} notifies, {} ok\nwfx missing {}\nFAILED:\n{}\nMISSING:\n{}\n", cov.len(), cov.len() - failed.len(), missing.len(), failed.join("\n"), missing.join("\n"));
    std::fs::write(dir.join("nano_summary2.txt"), &summary).ok();
    eprintln!("{summary}");
    for clip in ["8412", "8413", "8414", "8419", "8420", "8054"] {
        assert!(played.contains(&clip), "{clip} played: {played:?}");
    }
    assert!(failed.is_empty(), "{failed:#?}");
    assert!(missing.is_empty(), "{missing:#?}");
}

/// A form weapon's trail (`ead_key_trail0` of the clip, kh-host vfx.rs `form_trail_anchor`: the weapon's common
/// `key_trail0` between ITS vfx_top / vfx_bottom) is drawn under the hand keyblade's trail effect, not under its own name
/// (audit_a counts the Kingdom Key's `key_trail0` the same way): emitted and not refused by the host.
fn trail_ok(c: &audit_a::Covered) -> bool {
    c.emitted && c.want.seam_name().to_lowercase().ends_with("ead_key_trail0") && !c.resolved.contains("no effect of the pack")
}
