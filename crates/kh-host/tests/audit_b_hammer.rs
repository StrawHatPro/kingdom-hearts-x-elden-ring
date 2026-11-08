//! (AUDIT-B F-HAMMER) HYPER HAMMER -> DRILL PUNCH (Favorite Deputy) AND BOOM HAMMER -> CLOCK DRILL (Classic Tone), END TO
//! END: the real core -> the effect seam -> `kh_host::vfx::EffectHost` (main + XS + keyblade roots of the live pack, as kh-er
//! fx.rs reads them) and the sound seam -> `kh_host::audio::HostAudio` (the audio pack's routes, the swing / hit sets of the
//! weapon his attacks come from), recorded by AUDIT-A's `kh_host::audit_a::Session`. Each session runs offer ->
//! transformation -> the combo -> the second stage -> its combo -> FINISH_DR -> the end of the form, and every sound / voice
//! / effect notify of every clip Sora played is matched against what the seam emitted and what the hosts made of it
//! (`audit_a::coverage`, with KH3's effect-group rule applied: a row of another group than the play's is not due). The
//! traces go to build/sora_tmp/weapons/AUDIT-B/F-HAMMER/traces/. Skipped, saying so, without the pack.

use std::path::PathBuf;

use glam::Vec3;
use kh_core::{PadButton, StateId};
use kh_host::audit_a::{self, Covered, Session, Want};

fn traces() -> PathBuf {
    let dir = audit_a::project().join("build").join("sora_tmp").join("weapons").join("AUDIT-B").join("F-HAMMER").join("traces");
    std::fs::create_dir_all(&dir).ok();
    dir
}

/// The swing / hit sets of the weapon his attacks come from (kh-er kbdraw `select_sounds`: `Status::attack_model`).
fn select_sounds(s: &mut Session, base: &str) {
    let model = s.rig.sora.status().attack_model;
    let model = if model.is_empty() { base } else { model };
    if s.audio.engine.routes.weapon.as_deref() != Some(model) && !s.audio.engine.routes.select_weapon(model) {
        s.audio.engine.routes.weapon = Some(model.to_string());
    }
}

struct Run {
    s: Session,
    base: &'static str,
    /// (step, label) marks of the session's phases.
    marks: Vec<(u64, String)>,
    /// The play's effect group after each step (kh-core `Sora::effect_group`), by step.
    groups: Vec<(u64, i32)>,
    /// (AUDIT-B F-HAMMER) the actor's pitch after each step (Output::pitch), by step.
    pitches: Vec<(u64, f32)>,
}

impl Run {
    fn step(&mut self, held: &[PadButton]) -> StateId {
        select_sounds(&mut self.s, self.base);
        let state = self.s.step(held);
        self.groups.push((self.s.rig.out.step, self.s.rig.sora.effect_group()));
        self.pitches.push((self.s.rig.out.step, self.s.rig.out.pitch));
        state
    }

    fn idle(&mut self, n: usize) {
        for _ in 0..n {
            self.step(&[]);
        }
    }

    fn mark(&mut self, label: &str) {
        self.marks.push((self.s.rig.out.step, label.to_string()));
    }

    /// The gauge full, Triangle on the offered formchange, the transformation to its end.
    fn transform(&mut self, cmd: i32) {
        self.s.rig.sora.debug_add_style(100.0);
        self.idle(20);
        assert!(self.s.rig.sora.combat().situation.has(cmd), "{cmd:#x} offered");
        self.mark(&format!("transform {cmd:#x}"));
        self.step(&[PadButton::SITUATION]);
        for _ in 0..600 {
            if self.step(&[]) != StateId::Stylechange && self.s.rig.sora.combat().form.cmd == cmd {
                return;
            }
        }
        panic!("STYLECHANGE to {cmd:#x} did not end");
    }

    /// Attack presses until the chain is over.
    fn combo(&mut self, label: &str) {
        self.mark(label);
        for k in 0..1200 {
            let state = if k % 2 == 0 { self.step(&[PadButton::ATTACK]) } else { self.step(&[]) };
            if k > 30 && matches!(state, StateId::Idle | StateId::Run) {
                break;
            }
        }
        self.idle(30);
    }

    /// (AUDIT-B F-HAMMER) a jump, then Attack presses in the air until he lands and the chain is over (the air kinds).
    fn air_combo(&mut self, label: &str) {
        // the ground chain over first (a press buffered in it would take the jump)
        let mut calm = 0;
        for _ in 0..600 {
            calm = if matches!(self.step(&[]), StateId::Idle | StateId::Run) { calm + 1 } else { 0 };
            if calm >= 20 {
                break;
            }
        }
        self.mark(label);
        self.step(&[PadButton::JUMP]);
        self.idle(12);
        for k in 0..900 {
            let state = if k % 2 == 0 { self.step(&[PadButton::ATTACK]) } else { self.step(&[]) };
            if k > 60 && matches!(state, StateId::Idle | StateId::Run) {
                break;
            }
        }
        self.idle(30);
    }

    /// Triangle on FINISH_DR, until the finish and the form are over.
    fn finish(&mut self) {
        self.mark("FINISH_DR");
        self.step(&[PadButton::SITUATION]);
        for _ in 0..1200 {
            let state = self.step(&[]);
            if !matches!(state, StateId::Stylefinish | StateId::StylefinishDr) && self.s.rig.sora.combat().form.cmd == 0 {
                break;
            }
        }
        self.mark("form ended");
        self.idle(120);
    }

    fn group_at(&self, step: u64) -> i32 {
        self.groups.iter().find(|(s, _)| *s == step).map_or(0, |g| g.1)
    }
}

/// A covered notify, judged with the effect-group rule: (pass, line).
fn judged(r: &Run, c: &Covered) -> (bool, String) {
    let base = format!("step {:5} {} @{:.3} {} {}", c.step, c.clip, c.time, c.want.kind(), c.want.seam_name());
    if let Want::Effect { group, effect, .. } = &c.want {
        let current = r.group_at(c.step);
        let due = kh_core::notify::effect_group_passes(*group, *effect != "key_trail0", current);
        if !due {
            return (!c.emitted, format!("{base} group {group} (play group {current}): not due -> {}", if c.emitted { "EMITTED ANYWAY" } else { "not emitted (right)" }));
        }
        return (c.ok, format!("{base} group {group} (play group {current}) -> {} {}", if c.emitted { "emitted" } else { "NOT EMITTED" }, c.resolved));
    }
    (c.ok, format!("{base} -> {} {}", if c.emitted { "emitted" } else { "NOT EMITTED" }, c.resolved))
}

/// UE's socket lookup of an anchor's bone on the weapon's own skeleton / sockets (FName: no case); None = not there
/// (UE then attaches to the component: w_so052 / w_so112 / w_so113 name a socket `w_so052` they lack).
fn anchor_resolves(model: &str, bone: &str) -> Option<bool> {
    if bone.is_empty() {
        return Some(true);
    }
    let dir = audit_a::project().join("build").join("sora_pack").join("keyblades").join(model);
    let read = |f: &str| std::fs::read_to_string(dir.join(f)).ok().and_then(|t| serde_json::from_str::<serde_json::Value>(t.trim_start_matches('﻿')).ok());
    let (skeleton, sockets) = (read("skeleton.json")?, read("sockets.json")?);
    let bones = skeleton["skeleton"]["bones"].as_array().into_iter().flatten().filter_map(|b| b["name"].as_str());
    let socks = sockets["weapon_sockets"].as_object().into_iter().flat_map(|o| o.keys().map(String::as_str));
    Some(bones.chain(socks).any(|n| n.eq_ignore_ascii_case(bone)))
}

/// (AUDIT-B F-HAMMER) THE WEAPONS' / TRANSFORM UNITS' / THE CLOCK'S OWN CLIP NOTIFIES (`wnotify` rows of the sheet, played by
/// kh-core hammer::on_notifies -> ship::wnotify_pass): every row due in a step of the session (its form held, Sora's clip
/// its `clip`, its time in the step's notify window, its group the play's) must reach the seam once on its `kb:` anchor and
/// be drawn by the effect host / resolved by the audio route. (pass, line) per row due, in step order.
fn weapon_clip_rows(r: &Run) -> Vec<(bool, String)> {
    use kh_core::combat::forms::hammer::FORM_HAMMER;
    let form_of = |cmd: i32| match cmd {
        0x81 => "HM",
        0x84 => "DR",
        0x92 => "HM2",
        0x93 => "DR2",
        _ => "-",
    };
    let mut out = Vec::new();
    for row in &r.s.rows {
        let (from, to) = row.window;
        if (from, to) == (0.0, 0.0) {
            continue;
        }
        for w in FORM_HAMMER.iter().filter(|w| w.what == "wnotify" && w.clip == row.clip && w.form == form_of(row.form_cmd)) {
            let t = w.values.first().copied().unwrap_or(0.0);
            if !(t > from && t <= to || (from < 0.0 && t == 0.0)) {
                continue;
            }
            let anchor = w.flags.first().copied().unwrap_or("");
            let mut parts = anchor.split(':').skip(1);
            let (model, _mount, bone) = (parts.next().unwrap_or(""), parts.next().unwrap_or(""), parts.next().unwrap_or(""));
            let place = match anchor_resolves(model, bone) {
                Some(true) => "on its bone",
                Some(false) => "on the component (UE: no such socket)",
                None => "MODEL NOT IN THE PACK",
            };
            let head = format!("step {:5} {} @{t:.3} {} {}", row.step, row.clip, w.code, anchor);
            if w.code == 0 {
                let group = w.flags.get(3).and_then(|g| g.strip_prefix('g')).and_then(|g| g.parse::<i32>().ok()).unwrap_or(0);
                let name = w.effects.first().copied().unwrap_or("").to_lowercase();
                if !kh_core::notify::effect_group_passes(group, true, row.effect_group) {
                    let fired = row.effects.iter().any(|e| e.stop.is_none() && e.name == name && e.attach.contains(anchor));
                    out.push((!fired, format!("{head} effect {name} group {group} (play group {}): not due -> {}", row.effect_group, if fired { "EMITTED ANYWAY" } else { "not emitted (right)" })));
                    continue;
                }
                let hit = row.effects.iter().find(|e| e.stop.is_none() && e.name == name && e.attach.contains(anchor));
                let ok = hit.is_some_and(|e| e.drawn > 0) && place != "MODEL NOT IN THE PACK";
                let how = hit.map_or("NOT EMITTED".to_string(), |e| format!("emitted, drawn {} at actor {:?} (socket {:?}) {}", e.drawn, e.at_actor, e.socket_actor, e.notes.join("; ")));
                out.push((ok, format!("{head} effect {name} {} {place} -> {how}", w.flags.get(2).copied().unwrap_or(""))));
            } else {
                let name = w.sounds.first().copied().unwrap_or("");
                let short = name.to_lowercase();
                let sent = row.sounds.iter().any(|x| x.stop.is_none() && x.name.eq_ignore_ascii_case(name));
                let lines: Vec<String> = row.audio.iter().filter(|a| a.1.to_lowercase().ends_with(&short)).map(|a| format!("{} {}", a.0, a.2)).collect();
                let played = lines.iter().any(|l| l.starts_with("play") || l.starts_with("cancel"));
                out.push((sent && played, format!("{head} sound {name} {} -> {} {}", w.flags.get(2).copied().unwrap_or(""), if sent { "sent" } else { "NOT SENT" }, lines.join(" | "))));
            }
        }
    }
    out
}

/// One pair's session: the trace written; (failed lines, all lines).
fn session(key: &str, models: &[&str], base: &'static str, cmds: [i32; 2], name: &str) -> Option<(Vec<String>, usize)> {
    let mut s = Session::new(key, models, base, Some(Vec3::new(200.0, 0.0, 0.0)), name)?;
    // the XS root too (kh-er fx.rs reads it by default): Hyper Hammer's vw_so050 / vw_so052 effects are packed there
    let pack = audit_a::project().join("build").join("sora_pack");
    s.fx.load_xsi_root(&pack).expect("the XS effect root");
    let mut r = Run { s, base, marks: Vec::new(), groups: Vec::new(), pitches: Vec::new() };
    r.idle(5);
    r.transform(cmds[0]);
    r.idle(30);
    r.combo("combo (stage 1)");
    r.air_combo("air combo (stage 1)");
    r.transform(cmds[1]);
    r.idle(30);
    r.combo("combo (stage 2)");
    r.air_combo("air combo (stage 2)");
    // FINISH_DR with the dummy 6 m ahead (the warhead's reach)
    let o = r.s.rig.out.clone();
    let a = o.yaw.to_radians();
    let at = o.position + Vec3::new(a.cos(), a.sin(), 0.0) * 600.0;
    if let Some(d) = r.s.rig.dummy.as_mut() {
        d.info.position = Vec3::new(at.x, at.y, 0.0);
        d.info.center = Vec3::new(at.x, at.y, 90.0);
    }
    r.idle(10);
    assert_eq!(r.s.rig.sora.combat().situation.slot_b.map(|e| e.kind), Some(0x9E), "FINISH_DR on offer");
    r.finish();
    r.s.finish();
    let cov = audit_a::coverage(&r.s.rows);
    let mut text = String::new();
    for (step, label) in &r.marks {
        text.push_str(&format!("# mark step {step}: {label}\n"));
    }
    let mut failed = Vec::new();
    text.push_str("# notifies judged with the effect-group rule\n");
    for c in &cov {
        let (ok, line) = judged(&r, c);
        text.push_str(&format!("{} {line}\n", if ok { "PASS" } else { "FAIL" }));
        if !ok {
            failed.push(line);
        }
    }
    // (AUDIT-B F-HAMMER) the weapons' own clips
    let weapons = weapon_clip_rows(&r);
    text.push_str(&format!("# the form weapons' / transform units' / the Clock's own clip notifies ({} due, {} ok)
", weapons.len(), weapons.iter().filter(|w| w.0).count()));
    for (ok, line) in &weapons {
        text.push_str(&format!("{} {line}
", if *ok { "PASS" } else { "FAIL" }));
        if !ok {
            failed.push(line.clone());
        }
    }
    // (AUDIT-B F-HAMMER) the finisher's hits: their effects (the component's own m_HitEffects entry + its direct effect on a
    // phys shape) and the window's hit SE
    let want: &[&str] = if cmds[1] == 0x93 { &["vw_so114_fns_hit00", "vw_so110_str01"] } else { &["vw_so050_hit0", "vw_so050_str0"] };
    let finish: Vec<&audit_a::StepRow> = r.s.rows.iter().filter(|x| matches!(x.state, StateId::Stylefinish | StateId::StylefinishDr)).collect();
    for name in want {
        let n = finish.iter().map(|x| x.effects.iter().filter(|e| e.stop.is_none() && e.name == *name && e.drawn > 0).count()).sum::<usize>();
        let line = format!("finisher hit effect {name}: {n} drawn");
        text.push_str(&format!("{} {line}
", if n > 0 { "PASS" } else { "FAIL" }));
        if n == 0 {
            failed.push(line);
        }
    }
    let wrong = finish.iter().map(|x| x.effects.iter().filter(|e| e.stop.is_none() && e.name == "vw_so010_hit0").count()).sum::<usize>();
    text.push_str(&format!("{} finisher hits with the Kingdom Key's vw_so010_hit0: {wrong}
", if wrong == 0 { "PASS" } else { "FAIL" }));
    if wrong > 0 {
        failed.push(format!("{wrong} finisher hits with vw_so010_hit0"));
    }
    // (AUDIT-B F-HAMMER) Drill Punch's FINISH_DR: +0x1F4 bit 5 pitches him toward the target in M_6421 (the facing), levelled at
    // the catch (Turn((0, yaw, 0), 1.0)) before M_6423
    if cmds[1] == 0x84 {
        let pitch_of = |clip: &str| -> Vec<f32> { r.s.rows.iter().filter(|x| x.clip == clip && x.state == StateId::Stylefinish).filter_map(|x| r.pitches.iter().find(|p| p.0 == x.step).map(|p| p.1)).collect() };
        let (a, b) = (pitch_of("6421"), pitch_of("6423"));
        let most = a.iter().fold(0.0f32, |m, p| if p.abs() > m.abs() { *p } else { m });
        let level = b.iter().all(|p| p.abs() < 1.0e-3);
        let line = format!("FINISH_DR pitch: M_6421 up to {most:.2} deg over {} steps; M_6423 level on all {} steps: {level}", a.len(), b.len());
        text.push_str(&format!("{} {line}
", if level && !a.is_empty() { "PASS" } else { "FAIL" }));
        if !level || a.is_empty() {
            failed.push(line);
        }
    }
    // (AUDIT-B F-HAMMER) Clock Drill: the Clock's own notifies are anchored on Sora's component (`kb:w_so114:root:`), right only while
    // he stands at the Clock's root with its yaw (StyleFinishDR Tick: SetActorLocation(Sora, the Clock's root), no turn)
    if cmds[1] == 0x93 {
        let l421: Vec<&&audit_a::StepRow> = finish.iter().filter(|x| x.clip == "L421" && x.time < 3.8).collect(); // held while M_L421 < 3.8 s
        let (lo, hi) = l421.iter().fold((f32::MAX, f32::MIN), |(lo, hi), x| (lo.min(x.yaw), hi.max(x.yaw)));
        let moved = l421.windows(2).map(|w| (w[1].position - w[0].position).truncate().length()).fold(0.0f32, f32::max);
        let ok = !l421.is_empty() && hi - lo < 0.5 && moved < 1.0;
        let line = format!("Clock Drill: Sora's yaw in L421 (< 3.8 s) {lo:.2} .. {hi:.2} deg, largest step move {moved:.2} cm over {} steps (the Clock's root frame)", l421.len());
        text.push_str(&format!("{} {line}
", if ok { "PASS" } else { "FAIL" }));
        if !ok {
            failed.push(line);
        }
    }
    let hit4 = finish.iter().filter(|x| x.sounds.iter().any(|v| v.name == "HIT4") && x.audio.iter().any(|a| a.1 == "seam HIT4")).count();
    // (AUDIT-B W-DATA) Clock Drill's finisher hits are the CLOCK's (StylePawn w_so114): KH3 resolves the windows' alias HIT4 only
    // for a weapon actor (activation 0x140625500), so they play no hit SE (kh-host tests/audit_b_data.rs); Drill Punch's drill is
    // a weapon (w_so053_Wep): its HIT4 plays
    let clock = cmds[1] == 0x93;
    text.push_str(&format!("{} finisher hit SE HIT4 resolved on {hit4} step(s){}
", if (hit4 > 0) != clock { "PASS" } else { "FAIL" }, if clock { " (none due: the Clock is no weapon)" } else { "" }));
    if (hit4 > 0) == clock {
        failed.push(format!("finisher hit SE HIT4 on {hit4} step(s), the Clock {clock}"));
    }
    text.push_str(&audit_a::report(&r.s.rows));
    let dir = traces();
    std::fs::write(dir.join(format!("{name}.report.txt")), &text).ok();
    std::fs::copy(audit_a::path_in_logs(&format!("{name}.audio.csv")), dir.join(format!("{name}.audio.csv"))).ok();
    Some((failed, cov.len() + weapons.len()))
}

#[test]
fn favorite_deputy_hyper_hammer_drill_punch_trace() {
    let Some((failed, n)) = session("WEP_KEYBLADE_SO_03", &["w_so050", "w_so052", "w_so051", "w_so053"], "w_so050", [0x81, 0x84], "auditb_hammer_fd") else { return };
    assert!(failed.is_empty(), "{} of {n} notifies failed:\n{}", failed.len(), failed.join("\n"));
}

#[test]
fn classic_tone_boom_hammer_clock_drill_trace() {
    let Some((failed, n)) = session("WEP_KEYBLADE_SO_11", &["w_so110", "w_so112", "w_so111", "w_so113", "w_so114"], "w_so110", [0x92, 0x93], "auditb_hammer_ct") else { return };
    assert!(failed.is_empty(), "{} of {n} notifies failed:\n{}", failed.len(), failed.join("\n"));
}
