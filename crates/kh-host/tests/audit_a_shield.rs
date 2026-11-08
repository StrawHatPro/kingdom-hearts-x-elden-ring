//! (AUDIT-A SHIELD) Counter Shield (Hero's Origin, `form.SH` 0x89) and Frying Pan (Grand Chef, `form.SH2` 0x91), each
//! driven through the recorder (`kh_host::audit_a`: the real core, the real effect side, the real audio side with the
//! pack): offer -> StyleChange11 / 19 -> the COMBO_SH combo -> the raid chain (M_B413 -> M_B414 -> M_B415) -> the shield
//! guard held, one block (GuardChange, the counter window "Wrathful Fist" / "Flame Torrent") -> the grown attack 0x55 ->
//! two blocks (GuardChange, GuardChange2) and the guard walk -> 0x56 -> FINISH_SH (Pegasus + the Chariot with the thunder
//! extra / the Frying Pan) -> the end of the form. Every sound / voice / weapon-SE / effect notify of Sora's clips AND of
//! the finisher pawns' own clips (`pnotify` rows) must be emitted by the seam and resolved by the host. Reports:
//! build/sora_tmp/weapons/AUDIT-A/logs/audit_a_sh.report.txt / audit_a_sh2.report.txt (+ `.pawns.txt`); the inventory is
//! build/sora_tmp/weapons/AUDIT-A/SHIELD.md. Skipped without the pack.

use glam::{Vec2, Vec3};
use kh_core::combat::forms::shield::FORM_SHIELD;
use kh_core::enums::ETresDamageKind;
use kh_core::states::HitOnPlayer;
use kh_core::{PadButton, StateId, TargetId};
use kh_host::audit_a::{clips, coverage, path_in_logs, write_report, Session, StepRow};

const HERO: &[&str] = &["w_so030", "w_so036", "w_so035", "w_so032", "w_so031", "w_so033", "w_so037", "w_so034"];
const CHEF: &[&str] = &["w_so140", "w_so145", "w_so142", "w_so141", "w_so143", "w_so144"];

/// A hit from 150 cm in front of him.
fn hit_in_front(own: Vec3, yaw: f32) -> HitOnPlayer {
    let (s, c) = yaw.to_radians().sin_cos();
    let ahead = Vec3::new(c, s, 0.0);
    HitOnPlayer { direction: -ahead, causer_yaw: yaw + 180.0, ..HitOnPlayer::new(ETresDamageKind::Small, 30.0, TargetId(1), own + ahead * 150.0) }
}

/// What the session saw besides the notifies, for the assertions and SHIELD.md.
#[derive(Default, Debug)]
struct Seen {
    kinds: Vec<u8>,
    /// (counter window s, counter command) right after each block.
    counters: Vec<(f32, i32)>,
    grows: Vec<u8>,
    wear: i32,
    finish_subs: Vec<u8>,
    pawns: Vec<(u8, Vec<&'static str>)>,
    grown_invincible: bool,
    grown_slow: bool,
    ride_gauge: Vec<f32>,
    cameras: Vec<(u8, bool)>,
    /// (AUDIT-A SHIELD) per step: the samples Sora's play blends above 1e-5 (the pawns' M_B425 is AllAnimations: each fires)
    samples: std::collections::HashMap<u64, Vec<&'static str>>,
    /// (AUDIT-A SHIELD) per step: each thrown part's own clip clock after the step (placement::ThrownWeapon)
    thrown: std::collections::HashMap<u64, Vec<(&'static str, f32)>>,
    /// (AUDIT-A SHIELD) each thunder at its spawn: how far it is from Pegasus (the FINISH_SH_RUSH capsule rides Pegasus's actor)
    thunder_off: Vec<f32>,
}

/// (AUDIT-A SHIELD) What the step just taken blends and throws, for the expectations of the `all` / `thrown` rows.
fn note(s: &Session, seen: &mut Seen) {
    let Some(step) = s.rows.last().map(|r| r.step) else { return };
    seen.samples.insert(step, s.rig.out.anim_ticked.active_samples().into_iter().map(|x| x.0).collect());
    let thrown: Vec<(&'static str, f32)> = s.rig.sora.combat().thrown.iter().map(|t| (t.model, t.clip_time())).collect();
    if !thrown.is_empty() {
        seen.thrown.insert(step, thrown);
    }
}

/// (AUDIT-A SHIELD) `idle` that notes every step.
fn idle_noting(s: &mut Session, seen: &mut Seen, n: usize) {
    for _ in 0..n {
        s.step(&[]);
        note(s, seen);
    }
}

fn block(s: &mut Session, seen: &mut Seen) {
    let (own, yaw) = (s.rig.out.position, s.rig.out.yaw);
    s.rig.sora.take_hit(hit_in_front(own, yaw));
    for k in 0..110 {
        s.step(&[PadButton::GUARD]);
        if k == 2 {
            let a = s.rig.sora.actions();
            seen.counters.push((a.counter_window, a.counter_command));
            seen.grows.push(s.rig.sora.combat().form.grow);
        }
    }
}

fn grown_attack(s: &mut Session, seen: &mut Seen) {
    // released: GuardEnd runs out first
    assert!(s.until(200, |s| matches!(s.rig.out.state, StateId::Idle | StateId::Run)), "back on his feet");
    s.step(&[PadButton::ATTACK]);
    s.step(&[]);
    seen.grown_invincible |= s.rig.sora.status().damage_invalid;
    seen.grown_slow |= s.rig.sora.combat().form.world_slow;
    s.idle(260);
}

/// The whole session of one form.
fn run(weapon: &str, models: &[&str], sound: &str, cmd: i32, log: &str) -> Option<(Session, Seen)> {
    let mut s = Session::new(weapon, models, sound, Some(Vec3::new(250.0, 0.0, 0.0)), log)?;
    let mut seen = Seen::default();
    s.idle(5);
    // ---- the offer and the transformation
    s.rig.sora.debug_add_style(100.0);
    s.idle(15);
    assert_eq!(s.rig.sora.combat().situation.list.first().map(|e| e.kind), Some(cmd), "the formchange is offered");
    s.step(&[PadButton::SITUATION]);
    assert!(s.until(10, |s| s.rig.out.state == StateId::Stylechange), "StyleChange begins");
    assert!(s.until(400, |s| s.rig.out.state != StateId::Stylechange && s.rig.sora.combat().form.cmd != 0), "the form starts");
    assert_eq!(s.rig.sora.combat().form.cmd, cmd);
    seen.wear = i32::from(s.rig.sora.combat().form.wear_form);
    s.idle(30);
    // ---- the combo: press every other step until he is back on his feet
    let first = s.rig.sora.combat().history.len();
    for k in 0..400 {
        if k % 2 == 0 {
            s.step(&[PadButton::ATTACK]);
        } else {
            s.step(&[]);
        }
        note(&s, &mut seen); // (AUDIT-A SHIELD)
        if k > 40 && matches!(s.rig.out.state, StateId::Idle | StateId::Run) {
            break;
        }
    }
    idle_noting(&mut s, &mut seen, 90); // (AUDIT-A SHIELD) noted: the raid's throw
    // ---- the raid chain alone: press until 0x52 starts, then let M_B413 -> M_B414 -> M_B415 run out
    for _ in 0..600 {
        if s.rig.sora.combat().history.last() == Some(&0x52) && s.rig.out.state == StateId::ComboSh {
            break;
        }
        s.step(&[PadButton::ATTACK]);
        note(&s, &mut seen); // (AUDIT-A SHIELD)
        s.step(&[]);
        note(&s, &mut seen);
    }
    idle_noting(&mut s, &mut seen, 150); // (AUDIT-A SHIELD)
    seen.kinds = s.rig.sora.combat().history[first..].to_vec();
    // ---- the shield guard, one block: GuardChange, LV2, the counter window; then Wrathful Fist / Flame Torrent (0x55)
    for _ in 0..40 {
        s.step(&[PadButton::GUARD]);
    }
    block(&mut s, &mut seen);
    s.idle(30);
    grown_attack(&mut s, &mut seen);
    // ---- two blocks (GuardChange, GuardChange2), the walk, GuardEnd; then Wrathful Flurry / Flame Barrage (0x56)
    for _ in 0..40 {
        s.step(&[PadButton::GUARD]);
    }
    block(&mut s, &mut seen);
    block(&mut s, &mut seen);
    s.stick = Vec2::new(0.0, 1.0);
    for _ in 0..40 {
        s.step(&[PadButton::GUARD]);
    }
    s.stick = Vec2::ZERO;
    s.idle(60);
    assert_eq!(s.rig.sora.combat().form.grow, 3, "two blocks grew the weapon to LV3");
    grown_attack(&mut s, &mut seen);
    assert_eq!(s.rig.sora.combat().form.grow, 1, "the grown attack spent the level");
    // ---- FINISH_SH: the situation command, its extra pressed whenever offered, until the form is over
    s.step(&[PadButton::SITUATION]);
    let mut seen_thunders = 0; // (AUDIT-A SHIELD)
    for k in 0..3000 {
        let press = s.rig.sora.combat().situation.extra.is_some() && k % 2 == 0;
        if press {
            s.step(&[PadButton::SITUATION]);
        } else {
            s.step(&[]);
        }
        note(&s, &mut seen); // (AUDIT-A SHIELD) the ride's M_B425 samples
        // (AUDIT-A SHIELD) 0x141419A50: the thunder spawns at Pegasus's own location (its RootComponent), rotation zero
        let shots = &s.rig.sora.combat().shots;
        let thunders = shots.iter().filter(|x| x.name == "FINISH_SH_THUNDER").count();
        if thunders > seen_thunders {
            if let (Some(t), Some(rush)) = (shots.iter().rev().find(|x| x.name == "FINISH_SH_THUNDER"), shots.iter().find(|x| x.name == "FINISH_SH_RUSH")) {
                seen.thunder_off.push((t.position - rush.position).length());
            }
        }
        seen_thunders = thunders;
        let finish = s.rig.sora.finish();
        if let Some(f) = finish {
            if seen.finish_subs.last() != Some(&f.sub) {
                seen.finish_subs.push(f.sub);
                let pawns = kh_core::combat::forms::shield::finisher_pawns(finish, cmd).iter().map(|p| p.model).collect();
                seen.pawns.push((f.sub, pawns));
                if let Some(c) = kh_core::combat::forms::shield::finish_camera(finish, cmd) {
                    seen.cameras.push((f.sub, c.on_pegasus));
                }
            }
            if let Some(g) = kh_core::combat::forms::shield::ride_gauge(finish, cmd) {
                seen.ride_gauge.push(g);
            }
        }
        if k > 30 && finish.is_none() && !matches!(s.rig.out.state, StateId::Stylefinish | StateId::StylefinishSh) {
            break;
        }
    }
    s.idle(120);
    assert_eq!(s.rig.sora.combat().form.cmd, 0, "the finish ends the form");
    s.finish();
    Some((s, seen))
}

fn window_has(r: &StepRow, t: f32) -> bool {
    let (from, to) = r.window;
    if to >= from || !r.looping {
        t > from && t <= to || (from < 0.0 && t == 0.0)
    } else {
        t > from || t <= to
    }
}

/// The pawn rows whose sound asset the audio pack lacks (reported to AUDIT-A/SOUND_INBOX.md; the Chariot's se02003_013 / 019
/// were served by SOUND on 2026-11-03): none left.
const AWAITING_SOUND: &[&str] = &[];

/// Every pawn-clip notify (`pnotify` rows) of the form that the steps' clips fired: emitted and resolved. Returns the lines.
/// (AUDIT-A SHIELD) KH3's rule for each row of the step (AUDIT-B's shared pass runs them: the `wnotify` row of each `pnotify`
/// row): an `all` row (the pawns' M_B425 samples, AllAnimations) fires while its sample is in the blend, on Sora's window; a
/// `thrown` row on the thrown weapon's own clip clock, from the step after the throw (the weapon's first tick) until the
/// recall; any other row on Sora's heaviest sample's window. Returns the row's due time when it fires in step `r`.
fn due(p: &kh_core::combat::forms::shield::FormShieldRow, r: &StepRow, seen: &Seen) -> Option<f32> {
    let w = FORM_SHIELD.iter().find(|w| w.what == "wnotify" && w.id.strip_prefix("wnotify.") == p.id.strip_prefix("pnotify."))?;
    let t = w.values[0];
    if w.flags.contains(&"thrown") {
        let clock = |step: u64| seen.thrown.get(&step).and_then(|v| v.iter().find(|x| x.0 == w.flags[1])).map(|x| x.1);
        let (a, b) = (clock(r.step.checked_sub(1)?)?, clock(r.step)?);
        let hit = if b >= a { t > a && t <= b || (a == 0.0 && t == 0.0) } else { t > a || t <= b };
        return hit.then_some(t);
    }
    let blended = if w.flags.contains(&"all") { seen.samples.get(&r.step).is_some_and(|v| v.contains(&p.clip)) } else { p.clip == r.clip };
    (blended && r.window != (0.0, 0.0) && window_has(r, t)).then_some(t)
}

fn pawn_coverage(rows: &[StepRow], form: &str, seen: &Seen) -> (Vec<String>, usize) {
    let mut lines = Vec::new();
    let mut failed = 0;
    for r in rows.iter().filter(|r| r.form_cmd != 0) {
        for p in FORM_SHIELD.iter().filter(|p| p.what == "pnotify" && p.form == form) {
            // the weapons' rows pass KH3's group rule with his play's group (Frying Pan's J419 / J416: 2151 / 2152)
            let Some(t) = due(p, r, seen) else { continue };
            if p.flags[0] == "effect" && !kh_core::notify::effect_group_passes(p.group, true, r.effect_group) {
                continue;
            }
            let (ok, what) = if p.flags[0] == "effect" {
                let name = p.effects[0].rsplit(['.', '/']).next().unwrap_or("").to_lowercase();
                match r.effects.iter().find(|e| e.stop.is_none() && e.name == name) {
                    Some(e) => (e.drawn > 0, format!("effect {name} at {} drawn {} actor {:?} socket {:?} {}", e.attach, e.drawn, e.at_actor, e.socket_actor, e.notes.join("; "))),
                    None => (false, format!("effect {name}: NOT EMITTED")),
                }
            } else {
                let short = p.sounds[0].rsplit(['.', '/']).next().unwrap_or("").to_lowercase();
                let emitted = r.sounds.iter().any(|x| x.stop.is_none() && x.name == p.sounds[0]);
                let audio: Vec<String> = r.audio.iter().filter(|a| a.1.to_lowercase().ends_with(&short)).map(|a| format!("{} {} {}", a.0, a.2, a.3)).collect();
                // (AUDIT-A SHIELD) AllAnimations fires the same wing SE from several M_B425 samples in one step; KH3's SEAD start
                // macro of that sound then cancels the copies (the same sound started <= 0.067 s ago): one plays, as in KH3
                let deduped = r.audio.iter().any(|a| a.1.to_lowercase().ends_with(&short) && a.0 == "silent" && a.3.contains("start macro") && a.3.contains("cancelled"));
                let played = r.audio.iter().any(|a| a.1.to_lowercase().ends_with(&short) && (a.0 == "play" || a.0 == "voice")) || deduped;
                // (a row awaiting the SOUND inbox: emitted, the asset not packed yet)
                let awaiting = !played && emitted && AWAITING_SOUND.contains(&p.id) && audio.iter().any(|a| a.contains("not in the pack"));
                (emitted && (played || awaiting), format!("sound {short} {} -> {}{}", if emitted { "emitted" } else { "NOT EMITTED" }, audio.join(" | "), if awaiting { " [AWAITING SOUND_INBOX]" } else { "" }))
            };
            if !ok {
                failed += 1;
            }
            lines.push(format!("{} step {:5} {} {} @{:.3} {}", if ok { "PASS" } else { "FAIL" }, r.step, p.id, p.flags[1], t, what));
        }
    }
    (lines, failed)
}

fn check(s: &Session, seen: &Seen, log: &str, form: &str) {
    let text = write_report(&s.rows, log);
    let cov = coverage(&s.rows);
    assert!(!cov.is_empty(), "notifies fired:\n{text}");
    let failed: Vec<String> = cov.iter().filter(|c| !c.ok).map(|c| format!("{} {} @{:.3} {} {} -> {}", c.step, c.clip, c.time, c.want.kind(), c.want.seam_name(), c.resolved)).collect();
    let (pawn_lines, pawn_failed) = pawn_coverage(&s.rows, form, seen);
    let played: Vec<&str> = clips(&s.rows).into_iter().map(|c| c.0).collect();
    let summary = format!("{log}: {} notifies, {} failed; pawn notifies {} ({} failed); clips {played:?}\nseen kinds {:?} counters {:?} grows {:?} finish {:?} pawns {:?} cameras {:?}; steps noted {} (thrown {})\n", cov.len(), failed.len(), pawn_lines.len(), pawn_failed, seen.kinds, seen.counters, seen.grows, seen.finish_subs, seen.pawns, seen.cameras, seen.samples.len(), seen.thrown.len()); // (AUDIT-A SHIELD) not the per-step maps
    std::fs::write(path_in_logs(&format!("{log}.pawns.txt")), format!("{summary}{}\n", pawn_lines.join("\n"))).ok();
    eprintln!("{summary}");
    assert!(failed.is_empty(), "{log}: every notify row PASS; failed:\n{}", failed.join("\n"));
    assert!(pawn_failed == 0 && !pawn_lines.is_empty(), "{log}: every pawn notify PASS:\n{}", pawn_lines.join("\n"));
    // (AUDIT-A SHIELD) the thrown weapon's own clip ran on its clock; the pawns' M_B425 fired its blended samples (AllAnimations)
    assert!(pawn_lines.iter().any(|l| l.contains(".thrown@")), "{log}: the thrown weapon's notifies");
    if form == "SH" {
        assert!(pawn_lines.iter().any(|l| l.contains(".B425D") || l.contains(".B425U")), "{log}: a non-centre M_B425 sample fired");
    }
    // the moveset as KH3 decides it near one target, the raid, the grown attacks
    assert_eq!(&seen.kinds[..5], &[0x50, 0x51, 0x52, 0x53, 0x54], "the ground combo");
    for c in ["B413", "B414", "B415", "B418", "B426", "B285", "B286", "B288", "B289"] {
        assert!(played.contains(&c), "{c} not played: {played:?}");
    }
    assert!(seen.grown_invincible && seen.grown_slow, "the grown attacks: invincible, the world slowed");
    // the counter window after each block (FUN_1414145C0: 0x10 / 0x11 Counter Shield, 0x12 / 0x13 Frying Pan, 2 s)
    let base = if form == "SH" { 0x10 } else { 0x12 };
    assert_eq!(seen.counters.iter().map(|c| c.1).collect::<Vec<_>>(), [base, base, base + 1], "the counter window's command: {seen:?}");
    assert!(seen.counters.iter().all(|c| c.0 > 1.9), "a 2 s window: {seen:?}");
    // the grown part's aura (its ResetPose notify), drawn on the grown weapon, gone at the reset
    let auras: Vec<&str> = kh_core::combat::forms::shield::GROWN_AURA.iter().filter(|a| a.0 == form).map(|a| a.3.rsplit('.').next().unwrap_or("")).collect();
    for aura in auras {
        let lower = aura.to_lowercase();
        let spawned = s.rows.iter().flat_map(|r| r.effects.iter()).find(|e| e.stop.is_none() && e.name == lower);
        assert!(spawned.is_some_and(|e| e.drawn > 0), "{aura} drawn: {spawned:?}");
        let handle = spawned.map(|e| e.handle).unwrap_or(0);
        assert!(s.rows.iter().flat_map(|r| r.effects.iter()).any(|e| e.stop.is_some() && e.handle == handle), "{aura} ends with its weapon");
    }
}

#[test]
fn counter_shield_every_notify_passes() {
    let Some((s, seen)) = run("WEP_KEYBLADE_SO_01", HERO, "w_so030", 0x89, "audit_a_sh") else { return };
    check(&s, &seen, "audit_a_sh", "SH");
    assert_eq!(seen.wear, 3, "wear form 3");
    assert_eq!(seen.finish_subs, [1, 2, 3, 4, 5, 6, 7], "StyleFinishSH Tick's sub-steps");
    // Pegasus and the Chariot drawn from sub 3 (after sub 2 showed them) to 4; the Fix camera, then the ride camera
    let shown: Vec<u8> = seen.pawns.iter().filter(|p| !p.1.is_empty()).map(|p| p.0).collect();
    assert_eq!(shown, [3, 4]);
    assert!(seen.pawns.iter().filter(|p| !p.1.is_empty()).all(|p| p.1 == ["w_so036", "w_so035"]), "{:?}", seen.pawns);
    assert_eq!(seen.cameras, [(1, false), (2, false), (3, true), (4, true), (5, true)]);
    assert!(seen.ride_gauge.first().is_some_and(|g| *g > 0.99) && seen.ride_gauge.last().is_some_and(|g| *g < 0.02), "the ride gauge runs out");
    // the thunder's body and Pegasus's rush hit effect
    let fx: Vec<&str> = s.rows.iter().flat_map(|r| r.effects.iter().filter(|e| e.stop.is_none() && e.drawn > 0).map(|e| e.name.as_str())).collect();
    // (AUDIT-A SHIELD) every thunder at Pegasus (0x141419A50: its actor location, not the target)
    assert!(!seen.thunder_off.is_empty() && seen.thunder_off.iter().all(|d| *d < 0.01), "the thunders at Pegasus: {:?}", seen.thunder_off);
    for name in ["vw_so035_thd0", "vw_so035_dis_top", "vw_so035_dis_end"] {
        assert!(fx.contains(&name), "{name} drawn");
    }
}

#[test]
fn frying_pan_every_notify_passes() {
    let Some((s, seen)) = run("WEP_KEYBLADE_SO_10", CHEF, "w_so140", 0x91, "audit_a_sh2") else { return };
    check(&s, &seen, "audit_a_sh2", "SH2");
    assert_eq!(seen.wear, 4, "wear form 4");
    assert_eq!(seen.finish_subs, [2, 4, 5, 6], "slot 167's sub-steps");
    assert!(seen.cameras.is_empty(), "Frying Pan's finish has no camera of its own");
}
