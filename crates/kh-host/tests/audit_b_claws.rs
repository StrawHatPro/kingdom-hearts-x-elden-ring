//! (AUDIT-B F-CLAWS) AGILE CLAWS -> TWIN YO-YOS (Happy Gear) AND BLIZZARD CLAWS -> BLIZZARD BLADES (Crystal Snow), END TO END:
//! the real core -> the effect seam -> `kh_host::vfx::EffectHost` (main + XS + keyblade roots of the live pack, as kh-er fx.rs
//! reads them) and the sound seam -> `kh_host::audio::HostAudio` (the audio pack's routes, the swing / hit sets of the weapon
//! his attacks come from), recorded by AUDIT-A's `kh_host::audit_a::Session`. Each session runs offer -> the first stage's
//! transformation -> its combo -> the second stage -> its combo -> the finisher (FINISH_YO's giant yo-yo / FINISH_SK's
//! Chandelier) -> the end of the form. Checked:
//! - every sound / voice / effect notify of every clip SORA played (`audit_a::coverage`, KH3's effect-group rule: a row of
//!   another group than the play's is not due);
//! - every notify of the FORM WEAPONS' / the CHANDELIER'S own clips (kh-core `wfx.*` rows of sheets/port/sora-form-claws.json:
//!   tools/sora_form_claws.js) that the session reached: emitted by the seam and drawn / played by the hosts.
//!
//! The traces go to build/sora_tmp/weapons/AUDIT-B/F-CLAWS/traces/. Skipped, saying so, without the pack.

use std::path::PathBuf;

use glam::Vec3;
use kh_core::combat::forms::claws::FORM_CLAWS;
use kh_core::{PadButton, StateId};
use kh_host::audit_a::{self, Covered, Session, Want};

fn traces() -> PathBuf {
    let dir = audit_a::project().join("build").join("sora_tmp").join("weapons").join("AUDIT-B").join("F-CLAWS").join("traces");
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
    marks: Vec<(u64, String)>,
    groups: Vec<(u64, i32)>,
}

impl Run {
    fn step(&mut self, held: &[PadButton]) -> StateId {
        select_sounds(&mut self.s, self.base);
        let state = self.s.step(held);
        self.groups.push((self.s.rig.out.step, self.s.rig.sora.effect_group()));
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

    fn combo(&mut self, label: &str) {
        self.mark(label);
        for k in 0..1200 {
            let state = if k % 2 == 0 { self.step(&[PadButton::ATTACK]) } else { self.step(&[]) };
            if k > 30 && matches!(state, StateId::Idle | StateId::Run | StateId::Jump) {
                break;
            }
        }
        self.idle(30);
    }

    fn finish(&mut self, cmd: i32) {
        for _ in 0..30 {
            if self.s.rig.sora.combat().situation.slot_b.map(|e| e.kind) == Some(cmd) {
                break;
            }
            self.s.rig.sora.debug_add_style(100.0);
            self.idle(10);
        }
        assert_eq!(self.s.rig.sora.combat().situation.slot_b.map(|e| e.kind), Some(cmd), "the finish {cmd:#x} on offer");
        self.mark(&format!("finish {cmd:#x}"));
        self.step(&[PadButton::SITUATION]);
        for _ in 0..1500 {
            let state = self.step(&[]);
            if state != StateId::Stylefinish && self.s.rig.sora.combat().form.cmd == 0 {
                break;
            }
        }
        self.mark("form ended");
        self.idle(120);
    }

    fn group_at(&self, step: u64) -> i32 {
        self.groups.iter().find(|(s, _)| *s == step).map_or(0, |g| g.1)
    }

    /// The dummy `ahead` cm in front of him, `up` cm above the floor.
    fn place(&mut self, ahead: f32, up: f32) {
        let o = self.s.rig.out.clone();
        let a = o.yaw.to_radians();
        let at = o.position + Vec3::new(a.cos(), a.sin(), 0.0) * ahead;
        if let Some(d) = self.s.rig.dummy.as_mut() {
            d.info.position = Vec3::new(at.x, at.y, up);
            d.info.center = Vec3::new(at.x, at.y, 90.0 + up);
        }
    }
}

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

/// The weapons' / the Chandelier's own clip rows: per row the session's spawns / plays of its name (emitted and resolved).
fn weapon_rows(r: &Run, forms: &[&str]) -> (Vec<String>, Vec<String>) {
    let (mut lines, mut failed) = (Vec::new(), Vec::new());
    for row in FORM_CLAWS.iter().filter(|x| x.what == "wfx" && forms.contains(&x.form)) {
        let sound = row.flags.get(1) == Some(&"sound");
        let short = row.logical.rsplit(['/', '.']).next().unwrap_or(row.logical).to_lowercase();
        // the steps whose Sora clip is the row's (the weapon plays its own clip of it), up to the row's begin
        let begin: f32 = row.windows.first().and_then(|w| w.split('-').next()).and_then(|b| b.parse().ok()).unwrap_or(0.0);
        let played = r.s.rows.iter().any(|x| x.clip == row.clip && x.time >= begin);
        if !played {
            continue;
        }
        let group = row.values.first().copied().unwrap_or(0.0) as i32;
        let (emitted, resolved) = if sound {
            let e = r.s.rows.iter().any(|x| x.sounds.iter().any(|s| s.name.to_lowercase().ends_with(&short)));
            let p = r.s.rows.iter().any(|x| x.audio.iter().any(|a| a.1.to_lowercase().ends_with(&short) && (a.0 == "play" || a.0 == "voice")));
            (e, p)
        } else {
            let e = r.s.rows.iter().any(|x| x.effects.iter().any(|f| f.stop.is_none() && f.name == short));
            let d = r.s.rows.iter().any(|x| x.effects.iter().any(|f| f.stop.is_none() && f.name == short && f.drawn > 0));
            (e, d)
        };
        // a row of another group than any play of its clip had is not due (KH3's group rule)
        let due = sound || r.s.rows.iter().filter(|x| x.clip == row.clip).any(|x| kh_core::notify::effect_group_passes(group, true, r.group_at(x.step)));
        let ok = if due { emitted && resolved } else { !emitted };
        let line = format!("{} {} {} {} {} -> {}{}", if ok { "PASS" } else { "FAIL" }, row.id, if sound { "sound" } else { "effect" }, row.logical, row.windows.join(","), if !due { "not due (group)" } else if emitted { "emitted" } else { "NOT EMITTED" }, if due && emitted { if resolved { " resolved" } else { " NOT RESOLVED" } } else { "" });
        if !ok {
            failed.push(line.clone());
        }
        lines.push(line);
    }
    (lines, failed)
}

fn session(key: &str, models: &[&str], base: &'static str, cmds: [i32; 3], forms: &[&str], name: &str) -> Option<(Vec<String>, usize)> {
    let mut s = Session::new(key, models, base, Some(Vec3::new(200.0, 0.0, 0.0)), name)?;
    let pack = audit_a::project().join("build").join("sora_pack");
    s.fx.load_xsi_root(&pack).expect("the XS effect root");
    let mut r = Run { s, base, marks: Vec::new(), groups: Vec::new() };
    r.idle(5);
    r.transform(cmds[0]);
    r.idle(30);
    r.combo("combo (stage 1)");
    // the situational kinds: a far target (the dashes / Anchor Shot), a high one (the rises), then near again
    for (ahead, up, label) in [(700.0, 0.0, "far"), (200.0, 350.0, "high")] {
        r.place(ahead, up);
        r.idle(10);
        r.combo(&format!("combo (stage 1, {label})"));
    }
    r.place(200.0, 0.0);
    r.transform(cmds[1]);
    r.idle(30);
    r.combo("combo (stage 2)");
    for (ahead, up, label) in [(700.0, 0.0, "far"), (200.0, 350.0, "high")] {
        r.place(ahead, up);
        r.idle(10);
        r.combo(&format!("combo (stage 2, {label})"));
    }
    r.place(250.0, 0.0);
    r.idle(10);
    r.finish(cmds[2]);
    let finisher = if cmds[2] == 0x9C { "4420" } else { "7423" };
    assert!(r.s.rows.iter().any(|x| x.clip == finisher && x.state == StateId::Stylefinish), "the finisher {finisher} played");
    r.s.finish();
    let cov = audit_a::coverage(&r.s.rows);
    let mut text = String::new();
    for (step, label) in &r.marks {
        text.push_str(&format!("# mark step {step}: {label}\n"));
    }
    let mut failed = Vec::new();
    text.push_str("# Sora's clips' notifies, judged with the effect-group rule\n");
    for c in &cov {
        let (ok, line) = judged(&r, c);
        text.push_str(&format!("{} {line}\n", if ok { "PASS" } else { "FAIL" }));
        if !ok {
            failed.push(line);
        }
    }
    let (lines, wfailed) = weapon_rows(&r, forms);
    text.push_str(&format!("# the weapons' / the Chandelier's own clip rows reached ({} rows, {} failed)\n", lines.len(), wfailed.len()));
    for l in &lines {
        text.push_str(l);
        text.push('\n');
    }
    failed.extend(wfailed);
    text.push_str(&audit_a::report(&r.s.rows));
    let dir = traces();
    std::fs::write(dir.join(format!("{name}.report.txt")), &text).ok();
    std::fs::copy(audit_a::path_in_logs(&format!("{name}.audio.csv")), dir.join(format!("{name}.audio.csv"))).ok();
    Some((failed, cov.len() + lines.len()))
}

#[test]
fn happy_gear_agile_claws_twin_yo_yos_finish_yo_trace() {
    let Some((failed, n)) = session("WEP_KEYBLADE_SO_05", &["w_so060", "w_so061", "w_so062", "w_so063", "w_so064", "w_so065"], "w_so060", [0x8A, 0x82, 0x9C], &["CL", "YO"], "auditb_claws_hg") else { return };
    assert!(failed.is_empty(), "{} of {n} notifies failed:\n{}", failed.len(), failed.join("\n"));
}

#[test]
fn crystal_snow_blizzard_claws_blades_finish_sk_trace() {
    let Some((failed, n)) = session("WEP_KEYBLADE_SO_06", &["w_so080", "w_so081", "w_so082", "w_so083", "w_so084", "w_so085", "w_so086", "w_so087", "w_so088"], "w_so080", [0x90, 0x85, 0x9F], &["CL2", "SK"], "auditb_claws_cs") else { return };
    assert!(failed.is_empty(), "{} of {n} notifies failed:\n{}", failed.len(), failed.join("\n"));
}

/// The attack clips the sessions' Decide paths did not reach (KH3's Decide picks them by situation: a target below, the flow
/// attack, Anchor Shot's pull at a moving target, the blades' ground chain variants), each played whole in the form that owns
/// it (`Sora::debug_play_anim`, the form held) through the same recorder: every notify of Sora's clip and every row of the
/// form weapons' own clip judged as in the sessions. Trace `auditb_claws_sweep_<form>.report.txt`.
fn sweep(key: &str, models: &[&str], base: &'static str, stages: &[i32], clips: &[&str], forms: &[&str], name: &str) -> Option<(Vec<String>, usize)> {
    let mut s = Session::new(key, models, base, Some(Vec3::new(250.0, 0.0, 0.0)), name)?;
    let pack = audit_a::project().join("build").join("sora_pack");
    s.fx.load_xsi_root(&pack).expect("the XS effect root");
    let mut r = Run { s, base, marks: Vec::new(), groups: Vec::new() };
    r.idle(5);
    for cmd in stages {
        r.transform(*cmd);
        r.idle(20);
    }
    for clip in clips {
        let id = kh_core::AnimId::from_name(clip).unwrap_or_else(|| panic!("{clip} in the animation table"));
        r.mark(&format!("clip {clip}"));
        r.s.rig.sora.debug_play_anim(id, kh_core::PlayArgs::mode(0, 0.0));
        let mut seen = false;
        for _ in 0..400 {
            r.step(&[]);
            let now = r.s.rows.last().map(|x| x.clip);
            if now == Some(*clip) {
                seen = true;
            } else if seen {
                break;
            }
        }
        assert!(seen, "{clip} played");
        r.idle(10);
    }
    r.s.finish();
    let cov = audit_a::coverage(&r.s.rows);
    let mut text = String::new();
    for (step, label) in &r.marks {
        text.push_str(&format!("# mark step {step}: {label}\n"));
    }
    let mut failed = Vec::new();
    for c in cov.iter().filter(|c| clips.contains(&c.clip)) {
        let (ok, line) = judged(&r, c);
        text.push_str(&format!("{} {line}\n", if ok { "PASS" } else { "FAIL" }));
        if !ok {
            failed.push(line);
        }
    }
    let (lines, wfailed) = weapon_rows(&r, forms);
    for l in lines.iter().filter(|l| clips.iter().any(|c| l.contains(&format!(".{c}.")))) {
        text.push_str(l);
        text.push('\n');
    }
    failed.extend(wfailed);
    text.push_str(&audit_a::report(&r.s.rows));
    std::fs::write(traces().join(format!("{name}.report.txt")), &text).ok();
    Some((failed, cov.len() + lines.len()))
}

/// One sweep: (Keyblade item, its models, the sound model, the stages to enter, the clips, the sheet forms, the trace name).
type SweepRun<'a> = (&'a str, &'a [&'a str], &'static str, &'a [i32], &'a [&'a str], &'a [&'a str], &'a str);

#[test]
fn the_remaining_attack_clips_play_their_notifies() {
    let hg = ["w_so060", "w_so061", "w_so062", "w_so063", "w_so064", "w_so065"];
    let cs = ["w_so080", "w_so081", "w_so082", "w_so083", "w_so084", "w_so085", "w_so086", "w_so087", "w_so088"];
    let runs: [SweepRun; 4] = [
        ("WEP_KEYBLADE_SO_05", &hg, "w_so060", &[0x8A], &["C417", "C418"], &["CL"], "auditb_claws_sweep_cl"),
        ("WEP_KEYBLADE_SO_05", &hg, "w_so060", &[0x8A, 0x82], &["4415", "4416"], &["YO"], "auditb_claws_sweep_yo"),
        ("WEP_KEYBLADE_SO_06", &cs, "w_so080", &[0x90], &["C417", "C418"], &["CL2"], "auditb_claws_sweep_cl2"),
        ("WEP_KEYBLADE_SO_06", &cs, "w_so080", &[0x90, 0x85], &["7412", "7413", "7414", "7415", "7416", "7421", "7422"], &["SK"], "auditb_claws_sweep_sk"),
    ];
    let mut all = Vec::new();
    for (key, models, base, stages, clips, forms, name) in runs {
        let Some((failed, _)) = sweep(key, models, base, stages, clips, forms, name) else { return };
        all.extend(failed);
    }
    assert!(all.is_empty(), "{} failed:\n{}", all.len(), all.join("\n"));
}
