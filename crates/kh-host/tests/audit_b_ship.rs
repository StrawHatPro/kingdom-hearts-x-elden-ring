//! (AUDIT-B F-SHIP) HIGHWIND -> STORM FLAG (Wheel of Fate), END TO END: the real core -> the effect seam ->
//! `kh_host::vfx::EffectHost` (main + XS + keyblade roots of the live pack, as kh-er fx.rs reads them, with Wheel of
//! Fate's trail and sockets as kh-er sora/kbdraw.rs gives them) and the sound seam -> `kh_host::audio::HostAudio` (the audio
//! pack's routes, the swing / hit sets of the weapon his attacks come from), recorded by AUDIT-A's
//! `kh_host::audit_a::Session`. The session runs offer -> StyleChange14 -> COMBO_HL (E411 .. E414, the warp, E416's dive
//! and the spear rain) -> StyleChange15 -> COMBO_FL near (F411, F415 .. F418) -> the far flag throw (F412 -> F413 -> F414)
//! -> FINISH_FL (the Kraken, M_F420, M_1501) -> the end of the form. Checked:
//! - every sound / voice / effect notify of every clip Sora played, against what the seam emitted and what the hosts made of
//!   it (`audit_a::coverage`, with KH3's effect-group rule: a row of another group than the play's is not due);
//! - every notify of the WEAPONS' / KRAKEN'S own clips (kh-core `wnotify` rows: w_so091 StyleChange14 / 15, w_so092 M_E41x,
//!   w_so093 M_F413, w_so094 M_F420) at its time, emitted and drawn / played;
//! - the spear's and the ship's bones as the effect side resolves them (`kb:` anchors) against the generator's own FK (the
//!   sheet's shape tracks: tools/sora_form_ship.js), within 2 cm;
//! - the form weapons' trail (ead_key_trail0) drawn between the spear's own vfx_top / vfx_bottom;
//! - the BpEvents of the ca_key camera (E401, E416, F401, F418, F420).
//!
//! The traces go to build/sora_tmp/weapons/AUDIT-B/F-SHIP/traces/. Skipped, saying so, without the pack.

use std::path::PathBuf;

use glam::Vec3;
use kh_core::combat::forms::ship::FORM_SHIP;
use kh_core::host::EventKind;
use kh_core::{PadButton, StateId};
use kh_host::audit_a::{self, Covered, Session, Want};
use kh_host::vfx::Scene;

/// Wheel of Fate's models: the hand keyblade, the ship (transform unit), the spear, the flag, the Kraken.
const MODELS: [&str; 5] = ["w_so090", "w_so091", "w_so092", "w_so093", "w_so094"];

fn traces() -> PathBuf {
    let dir = audit_a::project().join("build").join("sora_tmp").join("weapons").join("AUDIT-B").join("F-SHIP").join("traces");
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

/// A shape track of the sheet (`track.<form>.<clip>.<group>`) at clip time `t`: its two ends, actor space.
fn track_at(clip: &str, group: &str, t: f32) -> Option<(Vec3, Vec3)> {
    let r = FORM_SHIP.iter().find(|r| r.what == "track" && r.clip == clip && r.logical == group)?;
    let frames = usize::try_from(r.code).ok().filter(|f| *f > 1)?;
    let f = (t / r.length).clamp(0.0, 1.0) * (frames - 1) as f32;
    let (i, w) = ((f.floor() as usize).min(frames - 1), f - f.floor());
    let j = (i + 1).min(frames - 1);
    let at = |k: usize| r.values[i * 7 + k] + (r.values[j * 7 + k] - r.values[i * 7 + k]) * w;
    Some((Vec3::new(at(0), at(1), at(2)), Vec3::new(at(3), at(4), at(5))))
}

/// One weapon-clip notify the session reached: (step, row id, what it asks, emitted, resolved).
struct WeaponNote {
    step: u64,
    id: &'static str,
    want: String,
    ok: bool,
    resolved: String,
}

struct Run {
    s: Session,
    /// (step, label) marks of the session's phases.
    marks: Vec<(u64, String)>,
    /// The play's effect group after each step (kh-core `Sora::effect_group`), by step.
    groups: Vec<(u64, i32)>,
    /// Every anim BpEvent the core sent (step, sequence, begin) and every state change (step, from, to).
    bp_events: Vec<(u64, &'static str, bool)>,
    states: Vec<(u64, StateId, StateId)>,
    /// The largest distance between a `kb:` bone the effect side resolves and the generator's FK of the same socket, per
    /// probe (name, cm, steps compared).
    fk: Vec<(&'static str, f32, usize)>,
    fk_first: Vec<String>,
    /// The spear trail's two points (actor frame) the first time it is live, and the steps it was live.
    trail: Option<(Vec3, Vec3)>,
    trail_steps: usize,
    /// The steps after which a Keyblade trail was live.
    trail_live: Vec<u64>,
    /// (AUDIT-A THROW) per step: each thrown part's own clip clock after the step (placement::ThrownWeapon)
    thrown: Vec<(u64, &'static str, f32)>,
}

impl Run {
    fn step(&mut self, held: &[PadButton]) -> StateId {
        select_sounds(&mut self.s, "w_so090");
        let state = self.s.step(held);
        let step = self.s.rig.out.step;
        self.groups.push((step, self.s.rig.sora.effect_group()));
        // (AUDIT-A THROW) the thrown parts' clocks, for the `thrown` rows' expectations
        let clocks: Vec<(u64, &'static str, f32)> = self.s.rig.sora.combat().thrown.iter().map(|t| (step, t.model, t.clip_time())).collect();
        self.thrown.extend(clocks);
        for e in &self.s.rig.out.events {
            match e.kind {
                EventKind::AnimBpEvent { seq, begin, .. } => self.bp_events.push((step, seq, begin)),
                EventKind::StateChange { from, to } => self.states.push((step, from, to)),
                _ => {}
            }
        }
        self.probe();
        state
    }

    /// The effect side's `kb:` bones against the sheet's FK, while the clips that carry those tracks play.
    fn probe(&mut self) {
        let out = self.s.rig.out.clone();
        let (seq, t) = (out.anim_ticked.body_samples()[0].0, out.anim_ticked.time);
        let none = |_: kh_core::TargetId| None;
        let scene = Scene { out: &out, sora: &self.s.rig.sora, tap: &self.s.audio.tap, target: &none, ground: Some(0.0), eye: out.position };
        let (sin, cos) = out.yaw.to_radians().sin_cos();
        let actor = |p: Vec3| {
            let d = p - out.position;
            Vec3::new(d.x * cos + d.y * sin, -d.x * sin + d.y * cos, d.z)
        };
        let probes: [(&'static str, &str, &str, &str, bool); 4] = [
            ("spear ATK_01 (E412 KB_StickM b)", "E412", "KB_StickM", "kb:w_so092:R_wep5_skt:ATK_01", true),
            ("spear ATK_00 (E412 KB_StickM a)", "E412", "KB_StickM", "kb:w_so092:R_wep5_skt:ATK_00", false),
            ("ship mast1 (E401 ChangeHL b)", "E401", "ChangeHL", "kb:w_so091:R_wep_skt:mast1", true),
            ("thrown flag ATK_01 (F413 KB_StickL b)", "F413", "KB_StickL", "kb:w_so093:R_wep6_skt:ATK_01", true),
        ];
        for (name, clip, group, anchor, end_b) in probes {
            // (the drawn pose of a play still blending in is not the clip's own pose: the sheet's FK is)
            // (the sheet's tracks are per frame of the 30 fps clips: compared on the frames, where neither side interpolates)
            let frame = t * 30.0;
            // (nor on a step whose drawn pose is already the next play's)
            let changed = out.anim.play_id != out.anim_ticked.play_id;
            // (AUDIT-A THROW) the flag KH3 THREW (F412's TresAnimNotify_ThrowWeapon) flies through F413 with its own TresAtkColl0:
            // its FK is the thrown part's shape (kh-core placement::thrown_shape, the generator's FK of the flag's own M_F413 on
            // its flying place), compared on THAT clip's 30 fps frames (not Sora's); the in-hand track is its place before the throw
            let thrown = self.s.rig.sora.combat().thrown.iter().find(|t| anchor.starts_with(&format!("kb:{}:", t.model))).copied();
            if seq != clip || changed || out.anim_ticked.weight < 1.0 || (thrown.is_none() && (frame - frame.round()).abs() > 1.0e-3) {
                continue;
            }
            let (want_a, want_b) = match thrown {
                Some(th) => {
                    let f = th.clip_time() * 30.0;
                    if (f - f.round()).abs() > 1.0e-3 {
                        continue;
                    }
                    let Some((a, b, _)) = kh_core::combat::forms::placement::thrown_shape(&[th], group) else { continue };
                    (actor(a), actor(b))
                }
                None => {
                    let Some((a, b)) = track_at(clip, group, t) else { continue };
                    (a, b)
                }
            };
            let want = if end_b { want_b } else { want_a };
            let got = actor(self.s.fx.socket_point(&scene, anchor));
            let d = got.distance(want);
            match self.fk.iter_mut().find(|f| f.0 == name) {
                Some(f) => {
                    (f.1, f.2) = (f.1.max(d), f.2 + 1);
                    if d > 2.0 {
                        self.fk_first.push(format!("{name} @{t:.3}: effect side {got:?}, FK {want:?}, off {d:.1}"));
                    }
                }
                None => {
                    let mount = actor(self.s.fx.socket_point(&scene, anchor.split(':').nth(2).unwrap_or("")));
                    self.fk_first.push(format!("{name} @{t:.3}: effect side {got:?}, FK {want:?}, mount {mount:?}"));
                    self.fk.push((name, d, 1));
                }
            }
        }
        // the form weapon's trail: on the spear's own sockets
        if !self.s.fx.trail_ids().is_empty() {
            self.trail_live.push(out.step);
        }
        if seq.starts_with("E41") && !self.s.fx.trail_ids().is_empty() {
            self.trail_steps += 1;
            if self.trail.is_none() {
                let top = actor(self.s.fx.socket_point(&scene, "kb:w_so092:R_wep5_skt:vfx_top"));
                let bottom = actor(self.s.fx.socket_point(&scene, "kb:w_so092:R_wep5_skt:vfx_bottom"));
                self.trail = Some((top, bottom));
            }
        }
    }

    fn idle(&mut self, n: usize) {
        for _ in 0..n {
            self.step(&[]);
        }
    }

    fn mark(&mut self, label: &str) {
        self.marks.push((self.s.rig.out.step, label.to_string()));
    }

    /// The dummy `ahead` cm in front of him, its centre `up` cm above its feet's 90.
    fn place(&mut self, ahead: f32, up: f32) {
        let o = self.s.rig.out.clone();
        let a = o.yaw.to_radians();
        let at = o.position + Vec3::new(a.cos(), a.sin(), 0.0) * ahead;
        if let Some(d) = self.s.rig.dummy.as_mut() {
            d.info.position = Vec3::new(at.x, at.y, up);
            d.info.center = Vec3::new(at.x, at.y, 90.0 + up);
        }
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
    fn combo(&mut self, label: &str) -> Vec<u8> {
        self.mark(label);
        let first = self.s.rig.sora.combat().history.len();
        for k in 0..1200 {
            let state = if k % 2 == 0 { self.step(&[PadButton::ATTACK]) } else { self.step(&[]) };
            if k > 30 && matches!(state, StateId::Idle | StateId::Run | StateId::Jump) {
                break;
            }
        }
        self.idle(30);
        self.s.rig.sora.combat().history[first..].to_vec()
    }

    fn group_at(&self, step: u64) -> i32 {
        self.groups.iter().find(|(s, _)| *s == step).map_or(0, |g| g.1)
    }

    /// Every `wnotify` row of the weapons' own clips due in a step (the step's clip and notify window), against the step's
    /// seam commands and the hosts' answers.
    fn weapon_coverage(&self) -> Vec<WeaponNote> {
        let mut out = Vec::new();
        for r in &self.s.rows {
            let (from, to) = r.window;
            // (AUDIT-A THROW) a `thrown` row (the flag's own M_F413) on the thrown part's clip clock, from the step after the
            // throw (the weapon's first tick) until the recall, whatever Sora plays (kh-core forms/ship.rs `wnotify_pass_dt`)
            let clock = |step: u64, model: &str| self.thrown.iter().find(|c| c.0 == step && c.1 == model).map(|c| c.2);
            let thrown_due = |w: &kh_core::combat::forms::ship::FormShipRow| {
                let model = w.flags[0].strip_prefix("kb:").and_then(|a| a.split(':').next()).unwrap_or("");
                let (Some(a), Some(b)) = (r.step.checked_sub(1).and_then(|p| clock(p, model)), clock(r.step, model)) else { return false };
                let t = w.values[0];
                if b >= a { t > a && t <= b || (a == 0.0 && t == 0.0) } else { t > a || t <= b }
            };
            for w in FORM_SHIP.iter().filter(|w| w.what == "wnotify" && (w.clip == r.clip || w.flags.contains(&"thrown"))) {
                let t = w.values[0];
                if w.flags.contains(&"thrown") {
                    if !thrown_due(w) {
                        continue;
                    }
                } else if (from, to) == (0.0, 0.0) || !(t > from && t <= to || (from < 0.0 && t == 0.0)) {
                    continue;
                }
                let (want, ok, resolved) = if w.code == 0 {
                    let name = w.effects[0].to_lowercase();
                    match r.effects.iter().find(|e| e.stop.is_none() && e.name == name) {
                        // (a system of the name alive after the step and no word from the effect side: drawn, though an older one of the same name may
                        // have ended in that step: the recorder counts the net change)
                        Some(e) => (format!("effect {name} on {}", w.flags[0]), e.drawn > 0 || (e.notes.is_empty() && e.at_actor.is_some()), format!("drawn {} at {} actor {:?}{}", e.drawn, e.attach, e.at_actor, if e.notes.is_empty() { String::new() } else { format!(" ({})", e.notes.join("; ")) })),
                        None => (format!("effect {name} on {}", w.flags[0]), false, "NOT EMITTED".to_string()),
                    }
                } else {
                    let name = w.sounds[0].to_lowercase();
                    let emitted = r.sounds.iter().any(|x| x.stop.is_none() && x.name.to_lowercase() == name);
                    let lines: Vec<String> = r.audio.iter().filter(|a| a.1.to_lowercase().ends_with(&name)).map(|a| format!("{} {} {}", a.0, a.2, a.3)).collect();
                    let played = r.audio.iter().any(|a| a.1.to_lowercase().ends_with(&name) && (a.0 == "play" || a.0 == "voice"));
                    (format!("{} {name} at {}", w.logical, w.flags[0]), emitted && played, if emitted { lines.join(" | ") } else { "NOT EMITTED".to_string() })
                };
                out.push(WeaponNote { step: r.step, id: w.id, want, ok, resolved });
            }
        }
        out
    }
}

/// A covered notify, judged with the effect-group rule: (pass, line).
fn judged(r: &Run, c: &Covered) -> (bool, String) {
    let base = format!("step {:5} {} @{:.3} {} {}", c.step, c.clip, c.time, c.want.kind(), c.want.seam_name());
    if let Want::Effect { group, effect, .. } = &c.want {
        let current = r.group_at(c.step);
        // the form weapon's trail (ead_key_trail0 -> vw_so090_loc0 on the spear's own sockets): emitted and a trail live after it
        if effect.contains("ead_key_trail0") {
            let live = r.trail_live.contains(&c.step);
            return (c.emitted && live, format!("{base} -> {} the weapon's trail {}", if c.emitted { "emitted" } else { "NOT EMITTED" }, if live { "live (on the form weapon's vfx_top / vfx_bottom)" } else { "NOT LIVE" }));
        }
        let due = kh_core::notify::effect_group_passes(*group, *effect != "key_trail0", current);
        if !due {
            return (!c.emitted, format!("{base} group {group} (play group {current}): not due -> {}", if c.emitted { "EMITTED ANYWAY" } else { "not emitted (right)" }));
        }
        return (c.ok, format!("{base} group {group} (play group {current}) -> {} {}", if c.emitted { "emitted" } else { "NOT EMITTED" }, c.resolved));
    }
    (c.ok, format!("{base} -> {} {}", if c.emitted { "emitted" } else { "NOT EMITTED" }, c.resolved))
}

/// Writes the session's report; (failed lines, all notifies checked).
fn write(r: &mut Run, name: &str) -> (Vec<String>, usize) {
    r.s.finish();
    let cov = audit_a::coverage(&r.s.rows);
    let weapon = r.weapon_coverage();
    let mut text = String::new();
    for (step, label) in &r.marks {
        text.push_str(&format!("# mark step {step}: {label}\n"));
    }
    text.push_str("# anim BpEvents (step, sequence, begin)\n");
    for (step, seq, begin) in &r.bp_events {
        text.push_str(&format!("bp step {step:5} {seq} {}\n", if *begin { "begin" } else { "end" }));
    }
    text.push_str("# state changes\n");
    for (step, from, to) in &r.states {
        text.push_str(&format!("state step {step:5} {from:?} -> {to:?}\n"));
    }
    text.push_str("# the effect side's form-weapon bones against the generator's FK (max cm over the steps)\n");
    for (name, d, n) in &r.fk {
        text.push_str(&format!("fk {name}: {d:.2} cm over {n} steps\n"));
    }
    for line in &r.fk_first {
        text.push_str(&format!("fk first {line}\n"));
    }
    text.push_str(&format!("# the spear's trail: live {} steps, first points (actor frame) {:?}\n", r.trail_steps, r.trail));
    let mut failed = Vec::new();
    text.push_str("# the weapons' / Kraken's own clip notifies\n");
    for w in &weapon {
        let line = format!("step {:5} {} {} -> {}", w.step, w.id, w.want, w.resolved);
        text.push_str(&format!("{} {line}\n", if w.ok { "PASS" } else { "FAIL" }));
        if !w.ok {
            failed.push(line);
        }
    }
    text.push_str("# Sora's clip notifies judged with the effect-group rule\n");
    for c in &cov {
        let (ok, line) = judged(r, c);
        text.push_str(&format!("{} {line}\n", if ok { "PASS" } else { "FAIL" }));
        if !ok {
            failed.push(line);
        }
    }
    text.push_str(&audit_a::report(&r.s.rows));
    let dir = traces();
    std::fs::write(dir.join(format!("{name}.report.txt")), &text).ok();
    std::fs::copy(audit_a::path_in_logs(&format!("{name}.audio.csv")), dir.join(format!("{name}.audio.csv"))).ok();
    (failed, cov.len() + weapon.len())
}

fn new_run(name: &str) -> Option<Run> {
    let mut s = Session::new("WEP_KEYBLADE_SO_09", &MODELS, "w_so090", Some(Vec3::new(200.0, 0.0, 0.0)), name)?;
    let pack = audit_a::project().join("build").join("sora_pack");
    s.fx.load_xsi_root(&pack).expect("the XS effect root");
    // Wheel of Fate in his hand as kh-er sora/kbdraw.rs tells the effect side: its trail (`kb.<KEY>.fx.trail`), its sockets
    // (w_so090/sockets.json) and its models' effects
    let trail = s.rig.sora.status().current_keyblade().and_then(|k| k.effect_name("fx.trail")).map(str::to_string);
    let sockets: Option<serde_json::Value> = std::fs::read_to_string(pack.join("keyblades").join("w_so090").join("sockets.json")).ok().and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok()).and_then(|j| j.get("weapon_sockets").cloned());
    let allowed: Vec<String> = MODELS.iter().map(|m| format!("v{m}")).collect();
    s.fx.set_keyblade(trail.as_deref(), sockets.as_ref(), &allowed);
    Some(Run { s, marks: Vec::new(), groups: Vec::new(), bp_events: Vec::new(), states: Vec::new(), fk: Vec::new(), fk_first: Vec::new(), trail: None, trail_steps: 0, trail_live: Vec::new(), thrown: Vec::new() /* (AUDIT-A THROW) */ })
}

/// The whole of both forms in one session.
#[test]
fn wheel_of_fate_highwind_storm_flag_trace() {
    let Some(mut r) = new_run("auditb_ship") else { return };
    r.idle(5);
    r.transform(0x8C);
    r.idle(30);
    r.place(200.0, 0.0);
    let hl = r.combo("COMBO_HL near");
    assert!(hl.starts_with(&[0x9C, 0x9D, 0x9E, 0x9F, 0xA0]), "Highwind's combo near one target: {hl:?}");
    r.idle(60);
    r.place(200.0, 0.0);
    r.transform(0x8D);
    r.idle(30);
    r.place(200.0, 0.0);
    let fl = r.combo("COMBO_FL near");
    assert!(fl.starts_with(&[0xA3, 0xA5, 0xA6, 0xA7, 0xA8]), "Storm Flag's combo near one target: {fl:?}");
    r.idle(120);
    r.place(800.0, 0.0);
    r.idle(5);
    r.mark("COMBO_FL far (the flag throw)");
    let first = r.s.rig.sora.combat().history.len();
    r.step(&[PadButton::ATTACK]);
    r.idle(150);
    let far = r.s.rig.sora.combat().history[first..].to_vec();
    assert_eq!(far.first(), Some(&0xA4), "the flag throw first: {far:?}");
    r.idle(30);
    r.place(200.0, 0.0);
    r.idle(5);
    assert_eq!(r.s.rig.sora.combat().situation.slot_b.map(|e| e.kind), Some(0xA6), "FINISH_FL on offer");
    r.mark("FINISH_FL");
    r.step(&[PadButton::SITUATION]);
    for _ in 0..1200 {
        let state = r.step(&[]);
        if !matches!(state, StateId::Stylefinish) && r.s.rig.sora.combat().form.cmd == 0 {
            break;
        }
    }
    r.mark("form ended");
    r.idle(180);
    let (failed, n) = write(&mut r, "auditb_ship");
    // the BpEvents of the camera blueprint, in their order
    let begins: Vec<&str> = r.bp_events.iter().filter(|e| e.2).map(|e| e.1).collect();
    assert_eq!(begins, ["E401", "E416", "F401", "F418", "F420"], "the ca_key camera's BpEvents");
    for (name, d, steps) in &r.fk {
        assert!(*d < 2.0 && *steps > 10, "{name}: the effect side's bone is {d} cm off the generator's FK ({steps} steps)");
    }
    assert_eq!(r.fk.len(), 4, "every bone probe ran: {:?}", r.fk);
    let (top, bottom) = r.trail.expect("the spear's trail was live");
    assert!(r.trail_steps > 20 && top.distance(bottom) > 50.0, "the trail spans the spear's blade: {top:?} {bottom:?}");
    assert!(failed.is_empty(), "{} of {n} notifies failed:\n{}", failed.len(), failed.join("\n"));
}

/// Highwind's two air kinds: the rise 0xA1 (M_E417) at a target high above him and the dive 0xA2 (M_E418) at a target far
/// below him (after a jump); their clips' and the spear's own notifies fire as KH3's.
#[test]
fn highwind_rise_and_dive_trace() {
    let Some(mut r) = new_run("auditb_ship_air") else { return };
    r.idle(5);
    r.transform(0x8C);
    r.idle(60);
    // the rise: the target 3 m up, 3 m ahead
    r.place(300.0, 300.0);
    r.idle(5);
    r.mark("rise (target above)");
    let first = r.s.rig.sora.combat().history.len();
    r.step(&[PadButton::ATTACK]);
    r.idle(120);
    let rise = r.s.rig.sora.combat().history[first..].to_vec();
    // the dive: a target 3 m below the floor he stands on (a pit), a jump, the attack at the top of it
    r.place(150.0, -300.0);
    r.idle(60);
    r.mark("dive (target below)");
    let first = r.s.rig.sora.combat().history.len();
    r.step(&[PadButton::JUMP]);
    r.idle(1);
    r.step(&[]);
    r.step(&[PadButton::JUMP]);
    r.idle(24);
    // (locked on, as the player would be: a target that deep is out of the soft target's reach)
    let lock = &mut r.s.rig.sora.combat_mut().lockon;
    (lock.locked, lock.current) = (true, kh_core::TargetId(1));
    r.step(&[PadButton::ATTACK]);
    r.idle(150);
    let dive = r.s.rig.sora.combat().history[first..].to_vec();
    let (failed, n) = write(&mut r, "auditb_ship_air");
    assert_eq!(rise.first(), Some(&0xA1), "the rise: {rise:?}");
    assert!(dive.contains(&0xA2), "the dive: {dive:?}");
    assert!(failed.is_empty(), "{} of {n} notifies failed:\n{}", failed.len(), failed.join("\n"));
}
