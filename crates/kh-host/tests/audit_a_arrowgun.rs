//! (AUDIT-A ARROWGUN) The gun formchanges through the recorder (`kh_host::audit_a`: the real core, the real effect side with
//! the keyblade root, the real audio side with the pack's routes and the Keyblade's swing / hit sets), one session per chain:
//! - Shooting Star: offer -> StyleChange2 (Double Arrowguns, wear form 2) -> COMBO_DW (0x44 0x45 0x46 0x47, the ring of 8)
//!   -> ShooterMode (RT held: ShooterModeStart, the gatling loop M_2416, ShooterModeEnd) -> the Sliding Shot / Back Slide
//!   -> the second offer -> StyleChange5 (Magic Launcher; Hunny Spout's forms: wear form 3) -> COMBO_BZ (0x4A 0x4B 0x4C Rising Blast) -> far:
//!   0x4D -> ShooterMode with the charge (the weapon glow, BZ_CRACKER / M_5416) -> FINISH_BZ (M_5417, BZ_BIG_BANG ->
//!   BigBangEx) -> the end of the form.
//! - Hunny Spout: the same through StyleChange16 / 17 (Hunny Blasters / Hunny Launcher, the honey effect groups), plus the
//!   Reflec guard and its Barrier Cracker counter (kind 0x25).
//!
//! Every sound / voice / weapon-SE / effect notify of every clip Sora played must be emitted by the seam AND resolved by the
//! host (an effect drawn, a sound asset played), judged with KH3's effect-group rule. Reports:
//! build/sora_tmp/weapons/AUDIT-A/logs/audit_a_arrowgun_{ss,hs}.report.txt (build/sora_tmp/weapons/AUDIT-A/ARROWGUN.md).
//! Skipped without the pack.

use glam::{Vec2, Vec3};
use kh_core::enums::ETresDamageKind;
use kh_core::states::HitOnPlayer;
use kh_core::{PadButton, StateId, TargetId};
use kh_host::audit_a::{self, Covered, Session, Want};

const SHOOTING_STAR: &[&str] = &["w_so020", "w_so021", "w_so022", "w_so023", "w_so024"];
const HUNNY_SPOUT: &[&str] = &["w_so040", "w_so041", "w_so042", "w_so043", "w_so044"];

struct Run {
    s: Session,
    marks: Vec<(u64, String)>,
    groups: Vec<(u64, i32)>,
    /// Effect spawns of the session that are not clip notifies (step, name, attach), for the report's own section.
    glow: Vec<(u64, String, String)>,
}

impl Run {
    fn step(&mut self, held: &[PadButton]) -> StateId {
        let state = self.s.step(held);
        self.groups.push((self.s.rig.out.step, self.s.rig.sora.effect_group()));
        if let Some(r) = self.s.rows.last() {
            for e in r.effects.iter().filter(|e| e.stop.is_none() && e.attach.contains("vfx_call")) {
                self.glow.push((r.step, e.name.clone(), e.attach.clone()));
            }
        }
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

    fn group_at(&self, step: u64) -> i32 {
        self.groups.iter().find(|(s, _)| *s == step).map_or(0, |g| g.1)
    }

    fn history(&self) -> Vec<u8> {
        self.s.rig.sora.combat().history.clone()
    }

    fn form(&self) -> i32 {
        self.s.rig.sora.combat().form.cmd
    }

    /// Fill the gauge, Triangle on the offer `cmd`, through STYLECHANGE into the form.
    fn transform(&mut self, cmd: i32, label: &str) {
        self.s.rig.sora.debug_add_style(100.0);
        self.idle(15);
        let list: Vec<i32> = self.s.rig.sora.combat().situation.list.iter().map(|e| e.kind).collect();
        assert!(list.contains(&cmd), "{cmd:#x} is offered: {list:x?}");
        self.mark(&format!("offer: Triangle on {label} ({cmd:#x})"));
        for _ in 0..40 {
            self.step(&[PadButton::SITUATION]);
            if self.step(&[]) == StateId::Stylechange {
                break;
            }
        }
        for _ in 0..600 {
            if self.step(&[]) != StateId::Stylechange && self.form() == cmd {
                break;
            }
        }
        assert_eq!(self.form(), cmd, "{label} began");
        self.mark(&format!("{label} on (wear form {})", self.s.rig.sora.combat().form.wear_form));
    }

    /// Attack on every other step until the chain is over; the kinds played.
    fn combo(&mut self, label: &str) -> Vec<u8> {
        self.mark(label);
        let first = self.history().len();
        let (mut attacking, mut finished) = (false, false);
        for k in 0..900 {
            finished |= self.s.rig.sora.combat().finish_end;
            let press = k % 2 == 0 && (k < 2 || attacking) && !finished;
            let state = if press { self.step(&[PadButton::ATTACK]) } else { self.step(&[]) };
            attacking = matches!(state, StateId::ComboDw | StateId::ComboBz | StateId::Attack);
            if k > 30 && !attacking && self.history().len() > first {
                break;
            }
        }
        self.idle(30);
        self.history()[first..].to_vec()
    }

    /// RT held until `done` (or `limit` steps), then released and the end played out.
    fn shooter(&mut self, label: &str, limit: usize, done: impl Fn(&Session) -> bool) {
        self.mark(label);
        let mut entered = false;
        for _ in 0..limit {
            entered |= self.step(&[PadButton::RightTrigger]) == StateId::Shootermode;
            if entered && done(&self.s) {
                break;
            }
        }
        assert!(entered, "{label}: ShooterMode began");
        self.mark(&format!("{label}: RT released"));
        for _ in 0..240 {
            if self.step(&[]) != StateId::Shootermode {
                break;
            }
        }
        self.idle(30);
    }

    /// The dummy `x` cm ahead of Sora (along his facing, on the floor).
    fn dummy_at(&mut self, x: f32) {
        let (out, (sin, cos)) = (self.s.rig.out.clone(), self.s.rig.out.yaw.to_radians().sin_cos());
        if let Some(d) = self.s.rig.dummy.as_mut() {
            let at = Vec3::new(out.position.x + cos * x, out.position.y + sin * x, 0.0);
            d.info.position = at;
            d.info.center = at + Vec3::Z * 90.0;
        }
    }

    fn finish_bz(&mut self) {
        self.idle(30);
        assert_eq!(self.s.rig.sora.combat().situation.slot_b.map(|e| e.kind), Some(0x9D), "FINISH_BZ on offer");
        let alive: Vec<String> = self.s.fx.world.live().iter().map(|l| l.name.clone()).collect();
        self.mark(&format!("effect systems alive before FINISH_BZ: {alive:?}"));
        self.mark("FINISH_BZ: Triangle");
        self.step(&[PadButton::SITUATION]);
        for _ in 0..900 {
            let state = self.step(&[]);
            if state != StateId::Stylefinish && self.form() == 0 {
                break;
            }
        }
        self.mark("the form ended");
        let alive: Vec<String> = self.s.fx.world.live().iter().map(|l| l.name.clone()).collect();
        self.mark(&format!("effect systems alive at the end of the form: {alive:?}"));
        assert_eq!(self.form(), 0, "the finish ends the form");
        self.idle(120);
    }
}

/// The sequence a logical clip name plays (its first sample).
fn seq(logical: &str) -> &'static str {
    let id = kh_core::AnimId::from_name(logical).unwrap_or_else(|| panic!("{logical}: no such clip"));
    kh_core::anim::SEQUENCES[id.data().samples[0].1].name
}

/// A covered notify, judged with the effect-group rule: (pass, line).
fn judged(r: &Run, c: &Covered) -> (bool, String) {
    let base = format!("step {:5} {} @{:.3} {} {}", c.step, c.clip, c.time, c.want.kind(), c.want.seam_name());
    if let Want::Effect { group, effect, socket, .. } = &c.want {
        let current = r.group_at(c.step);
        if !kh_core::notify::effect_group_passes(*group, *effect != "key_trail0", current) {
            return (!c.emitted, format!("{base} socket {socket} group {group} (play group {current}): not due -> {}", if c.emitted { "EMITTED ANYWAY" } else { "not emitted (right)" }));
        }
        return (c.ok, format!("{base} socket {socket} group {group} (play group {current}) -> {} {}", if c.emitted { "emitted" } else { "NOT EMITTED" }, c.resolved));
    }
    let extra = match &c.want {
        Want::Voice { random, .. } if *random > 1 => format!(" random x{random}"),
        _ => String::new(),
    };
    (c.ok, format!("{base}{extra} -> {} {}", if c.emitted { "emitted" } else { "NOT EMITTED" }, c.resolved))
}

/// (AUDIT-A ARROWGUN) Whether time `t` of Sora's clip falls in a step's notify window (`(from, to]`, `from < 0` on the
/// play's first update; a wrapped loop `(from, length] + (0, to]`: a state row begins again in the new loop only), as
/// forms/ship.rs `wnotify_pass` reads it.
fn due_in(t: f32, state: bool, r: &audit_a::StepRow) -> bool {
    let (from, to) = r.window;
    if r.looping && to < from {
        t <= to || (!state && t > from)
    } else {
        (t > from && t <= to) || (from < 0.0 && t == 0.0)
    }
}

/// (AUDIT-A ARROWGUN) A `wnotify` row of the arrowgun sheet (a gun's / launcher's / transform unit's OWN clip notify, AUDIT-B's
/// seam): every step of the session whose notify window of Sora's `clip`, in the row's form, passes the row's time must
/// carry the seam's spawn on the row's `kb:` anchor and the effect side must draw it (or the play's effect group refuses
/// it and nothing is spawned: KH3's group rule), or carry its sound and the audio side must resolve it to a sound asset; a
/// state row's effect / sound must be stopped at its end time or with the play. None: the row's clip never played in its
/// form in this session.
fn judged_wnotify(s: &Session, row: &kh_core::combat::forms::arrowgun::FormArrowgunRow) -> Option<(bool, String)> {
    let cmd = match row.form {
        "DW" => 0x80,
        "BZ" => 0x83,
        "DW2" => 0x8E,
        _ => 0x8F,
    };
    let v = |i: usize| row.values.get(i).copied().unwrap_or(0.0);
    let (t, end) = (v(0), v(1));
    let anchor = row.flags.first().copied().unwrap_or("");
    let state = row.flags.get(2) == Some(&"state");
    let plays: Vec<(usize, &audit_a::StepRow)> = s.rows.iter().enumerate().filter(|(_, r)| r.clip == row.clip && r.form_cmd == cmd).collect();
    if plays.is_empty() {
        return None;
    }
    let due: Vec<(usize, &audit_a::StepRow)> = plays.iter().copied().filter(|(_, r)| due_in(t, state, r)).collect();
    let base = format!("{} {} {}@{t:.4}{} {anchor} {}", row.id, row.logical, row.clip, if state { format!("..{end:.4}") } else { String::new() }, row.form);
    if due.is_empty() {
        let first = plays.first().map(|(_, r)| format!("first step {} window {:?}", r.step, r.window)).unwrap_or_default();
        return Some((false, format!("{base} -> NO STEP passes its time ({first})")));
    }
    // the stop of a state row's handle: (step, Sora's clip time, same play) of the first later Stop
    // (DRILL-IDLE) an effect row's handle among the EffectStops, a sound row's among the SoundStops: the core counts effect and
    // sound handles apart (pawn.rs `next_effect` / `next_sound`), so an effect stop of the same number is not this sound's
    let stop_of = |from: usize, handle: u32, play: u32, sound: bool| s.rows[from..].iter().find(|x| if sound { x.sounds.iter().any(|e| e.stop.is_some() && e.handle == handle) } else { x.effects.iter().any(|e| e.stop.is_some() && e.handle == handle) }).map(|x| (x.step, x.time, x.play_id == play, x.looping && x.window.1 < x.window.0));
    let stop_ok = |stop: Option<(u64, f32, bool, bool)>| match stop {
        Some((step, time, same, wrapped)) => (!same || wrapped || time >= end - 1.0e-4, format!("stopped step {step} at {time:.3}{}", if !same { " (play changed)" } else if wrapped { " (the loop wrapped)" } else { "" })),
        None => (false, "NEVER STOPPED".to_string()),
    };
    let mut lines = Vec::new();
    let mut ok = true;
    for (i, r) in due {
        if row.code == 0 {
            let group = row.flags.get(3).and_then(|g| g.strip_prefix('g')).and_then(|g| g.parse::<i32>().ok()).unwrap_or(0);
            let name = row.logical.to_lowercase();
            let hit = r.effects.iter().find(|e| e.stop.is_none() && e.name == name && e.attach.contains(&format!("\"{anchor}\"")));
            if !kh_core::notify::effect_group_passes(group, true, r.effect_group) {
                ok &= hit.is_none();
                lines.push(format!("step {} group {group} (play group {}): not due -> {}", r.step, r.effect_group, if hit.is_some() { "EMITTED ANYWAY" } else { "not emitted (right)" }));
                continue;
            }
            match hit {
                Some(e) if e.drawn > 0 => {
                    let (stopped, how) = if state { stop_ok(stop_of(i + 1, e.handle, r.play_id, false)) } else { (true, "instant".to_string()) };
                    ok &= stopped;
                    lines.push(format!("step {} t {:.3} emitted drawn {} at actor {:?} (anchor point {:?}), {how}", r.step, r.time, e.drawn, e.at_actor.map(|p| p.round()), e.socket_actor.map(|p| p.round())));
                }
                Some(e) => {
                    ok = false;
                    lines.push(format!("step {} emitted, NOT DRAWN {:?}", r.step, e.notes));
                }
                None => {
                    ok = false;
                    lines.push(format!("step {} NOT EMITTED", r.step));
                }
            }
        } else {
            let name = row.logical;
            let short = name.rsplit(['/', '.']).next().unwrap_or(name);
            let emitted = r.sounds.iter().find(|x| x.stop.is_none() && x.name == name);
            let heard = r.audio.iter().find(|a| (a.0 == "play" || a.0 == "random-voice") && (a.1.contains(short) || a.2.contains(short) || a.1.contains(name)));
            match (emitted, heard) {
                (Some(e), Some(a)) => {
                    let (stopped, how) = if state { stop_ok(stop_of(i + 1, e.handle, r.play_id, true)) } else { (true, "instant".to_string()) };
                    ok &= stopped;
                    lines.push(format!("step {} t {:.3} emitted at {}, {} {} {} ({how})", r.step, r.time, e.attach, a.0, a.2, a.3));
                }
                (Some(_), None) => {
                    ok = false;
                    lines.push(format!("step {} emitted, NOT RESOLVED {:?}", r.step, r.audio));
                }
                (None, _) => {
                    ok = false;
                    lines.push(format!("step {} NOT EMITTED", r.step));
                }
            }
        }
    }
    Some((ok, format!("{base} -> {}", lines.join("; "))))
}

/// (AUDIT-A ARROWGUN) The own-clip rows of the chain's forms, judged: (judged, failed lines, rows of the forms not played).
fn wnotify_report(s: &Session, forms: &[&str], text: &mut String) -> (usize, Vec<String>, Vec<&'static str>) {
    text.push_str("# the guns' / launchers' / transform units' OWN clip notifies (wnotify rows, AUDIT-B's seam; forms/ship.rs wnotify_pass)\n");
    let (mut judged, mut failed, mut missing) = (0, Vec::new(), Vec::new());
    for row in kh_core::combat::forms::arrowgun::FORM_ARROWGUN.iter().filter(|r| r.what == "wnotify" && forms.contains(&r.form)) {
        match judged_wnotify(s, row) {
            Some((ok, line)) => {
                judged += 1;
                text.push_str(&format!("{} {line}\n", if ok { "PASS" } else { "FAIL" }));
                if !ok {
                    failed.push(line);
                }
            }
            None => {
                text.push_str(&format!("FAIL {} {}: clip not played in this session\n", row.id, row.clip));
                missing.push(row.id);
            }
        }
    }
    // the launchers' ChangeAttachParent windows (the hosts' part placement: AUDIT-A PLACEMENT's generic reader, kh-core
    // forms/placement.rs `part_parent`): kh_root while Sora's FINISH_BZ clip runs, the weapons' own clip rows of these forms
    use kh_core::combat::forms::placement::{part_parent, FORM_PLACEMENT};
    for (_, cmd, model) in [("BZ", 0x83, "w_so024"), ("BZ2", 0x8F, "w_so044")].into_iter().filter(|f| forms.contains(&f.0)) {
        let Some(p) = FORM_PLACEMENT.iter().find(|r| r.what == "parent" && r.model == model && r.cmds.contains(&cmd)) else {
            failed.push(format!("{model}: no ChangeAttachParent row in sora-form-placement"));
            continue;
        };
        let seq = p.names.first().and_then(|n| n.strip_prefix("M_")).unwrap_or(p.clip);
        let steps: Vec<(u64, f32, Option<&str>)> = s.rows.iter().filter(|x| x.clip == seq && x.form_cmd == cmd).map(|x| (x.step, x.time, part_parent(x.form_cmd, model, x.clip, x.time))).collect();
        let hung = steps.iter().filter(|x| x.2 == Some("kh_root")).count();
        let ok = !steps.is_empty() && steps.iter().all(|x| x.2.is_some() == (x.1 >= p.values[0] && x.1 < p.values[1]));
        let line = format!("{} {model} (its clip {}) on {} {:.4}..{:.4}: {hung} of {} steps of Sora's {seq} hang it on kh_root", p.id, p.clip, p.logical, p.values[0], p.values[1], steps.len());
        text.push_str(&format!("{} {line}
", if ok { "PASS" } else { "FAIL" }));
        if ok {
            judged += 1;
        } else {
            failed.push(line);
        }
    }
    (judged, failed, missing)
}

fn report(r: &mut Run, name: &str, forms: &[&str]) -> (usize, Vec<String>, usize) {
    r.s.finish();
    let cov = audit_a::coverage(&r.s.rows);
    let mut text = String::new();
    for (step, label) in &r.marks {
        text.push_str(&format!("# mark step {step}: {label}\n"));
    }
    text.push_str("# the weapon glow while charging (pawn vt+0x1468 -> the launcher's m_CmnChargeEffect on its vfx_call)\n");
    for (step, effect, attach) in &r.glow {
        text.push_str(&format!("step {step:5} {effect} at {attach}\n"));
    }
    let mut failed = Vec::new();
    text.push_str("# notifies judged with the effect-group rule\n");
    for c in &cov {
        let (ok, line) = judged(r, c);
        text.push_str(&format!("{} {line}\n", if ok { "PASS" } else { "FAIL" }));
        if !ok {
            failed.push(line);
        }
    }
    // (AUDIT-A ARROWGUN) the weapons' own clips
    let (own, own_failed, missing) = wnotify_report(&r.s, forms, &mut text);
    failed.extend(own_failed);
    failed.extend(missing.iter().map(|m| format!("{m}: its clip never played in its form")));
    text.push_str(&audit_a::report(&r.s.rows));
    std::fs::write(audit_a::path_in_logs(&format!("{name}.report.txt")), &text).ok();
    (cov.len(), failed, own)
}

/// One chain: (keyblade item, its models, the sound model, first / second stage commands, the transformation clips).
fn chain(weapon: &str, models: &[&str], sound: &str, (dw, bz): (i32, i32), (dw_clip, bz_clip): (&str, &str), log: &str) {
    let Some(s) = Session::new(weapon, models, sound, Some(Vec3::new(250.0, 0.0, 0.0)), log) else { return };
    let mut r = Run { s, marks: Vec::new(), groups: Vec::new(), glow: Vec::new() };
    let hunny = dw == 0x8E;
    r.idle(5);
    // ---- the first stage
    r.transform(dw, if hunny { "Hunny Blasters" } else { "Double Arrowguns" });
    let wear = if hunny { 3 } else { 2 };
    assert_eq!(r.s.rig.sora.combat().form.wear_form, wear, "the wear form (sora-keyblade-forms: Wisdom 2 / Solid 3)");
    assert!(audit_a::clips(&r.s.rows).iter().any(|c| c.0 == seq(dw_clip)), "{dw_clip} played: {:?}", audit_a::clips(&r.s.rows));
    r.idle(20);
    let kinds = r.combo("COMBO_DW");
    assert_eq!(kinds, [0x44, 0x45, 0x46, 0x47], "COMBO_DW near one target");
    // ShooterMode: the gatling loop for two seconds
    let from = r.s.rows.len();
    r.shooter("ShooterMode (DW: Auto-Shot)", 200, |s| s.rows.len() > from + 150);
    if !hunny {
        // (AUDIT-A ARROWGUN) the arrowguns' own "ShooterMode" rows (w_so022 / w_so023 clip 2405: the gatling barrels'
        // vw_so021_gat2 on R/L_gatoA4 / B4, 0.0001 - 2.0): KH3 never plays Sora's logical ShooterMode in 0x80 (ShooterMode
        // Tick 0x1413B0DC0 sub 1: field_0xb8 == 0 -> DW_GATLING + M_2416, sub 3; only the launchers play "ShooterMode"
        // (sub 1 / 4); the only other L"ShooterMode" of the exe is the state's name in its ctor 0x141506490), so the port
        // plays none there either. Forced here once (a host's play-anim) to prove the rows are wired and drawn.
        r.mark("FORCED (data check, not KH3 play): Sora's 2405 (DW ShooterMode) via debug_play_anim, 2.1 s");
        let id = kh_core::combat::forms::arrowgun::replaced_clip(0x80, "ShooterMode").and_then(kh_core::AnimId::from_name).expect("DW ShooterMode");
        r.s.rig.sora.debug_play_anim(id, kh_core::PlayArgs::mode(0, 0.0));
        r.idle(130);
    }
    // the Sliding Shot (stick to his right) and the Back Slide (no stick) in battle; Hunny Blasters guard instead
    r.s.rig.sora.force_battle(true);
    r.dummy_at(600.0);
    r.idle(10);
    r.mark("Sliding Shot (guard + stick right)");
    r.s.stick = Vec2::new(1.0, 0.0);
    r.step(&[PadButton::GUARD]);
    r.s.stick = Vec2::ZERO;
    r.idle(60);
    if !hunny {
        r.mark("Back Slide (guard, no stick)");
        r.step(&[PadButton::GUARD]);
        r.idle(60);
    } else {
        // the Reflec guard (AnimSet16 G279) and its counter Barrier Cracker (0x25) out of a guarded hit
        r.s.rig.sora.status_mut().abilities.set(kh_core::enums::ETresAbilityKind::ReflectGuard, 1, 1);
        r.mark("Reflec guard (guard, no stick) + a guarded hit + Attack = Barrier Cracker");
        r.step(&[PadButton::GUARD]);
        r.idle(8);
        r.dummy_at(200.0);
        let me = r.s.rig.out.position;
        let at = r.s.rig.dummy.map_or(Vec3::ZERO, |d| d.info.position);
        let d = (Vec3::new(me.x, me.y, 0.0) - at).normalize_or_zero();
        r.s.rig.sora.take_hit(HitOnPlayer { direction: d, causer_yaw: d.y.atan2(d.x).to_degrees(), ..HitOnPlayer::new(ETresDamageKind::Small, 30.0, TargetId(1), at) });
        r.idle(1);
        let first = r.history().len();
        r.step(&[PadButton::ATTACK]);
        r.idle(120);
        assert_eq!(r.history()[first..].first(), Some(&0x25), "the Hunny guard counter is Barrier Cracker (0x25)");
    }
    r.s.rig.sora.force_battle(false);
    r.idle(30);
    // ---- the second stage, offered inside the first
    r.transform(bz, if hunny { "Hunny Launcher" } else { "Magic Launcher" });
    assert_eq!(r.s.rig.sora.combat().form.wear_form, wear, "the wear form stays");
    assert!(audit_a::clips(&r.s.rows).iter().any(|c| c.0 == seq(bz_clip)), "{bz_clip} played");
    r.idle(20);
    r.dummy_at(250.0);
    r.idle(5);
    assert_eq!(r.combo("COMBO_BZ near"), [0x4A, 0x4B, 0x4C], "COMBO_BZ near: the swings, Rising Blast");
    r.dummy_at(1200.0);
    r.idle(20);
    let far = r.combo("COMBO_BZ far");
    assert_eq!(far.first(), Some(&0x4D), "far: the bomb shot");
    r.idle(120);
    r.dummy_at(600.0);
    // ShooterMode with the charge: held to the full charge and its BZ_CRACKER (M_5416)
    r.shooter("ShooterMode (BZ: Charged Shot)", 400, |s| audit_a::clips(&s.rows).iter().any(|c| c.0 == "5416" || c.0 == "M_5416"));
    assert!(r.glow.iter().any(|g| g.1.contains(if hunny { "vw_so044_bom_chg00" } else { "vw_bom_lv0" })), "the launcher glows while charging: {:?}", r.glow);
    // the shot / End end it (EndEffect of group 0x802 = DeactivateSystem: LOOPEND): nothing of it is alive 2 s later
    let glow_name = if hunny { "vw_so044_bom_chg00" } else { "vw_bom_lv0" };
    let glow_handles: Vec<u32> = r.s.rows.iter().flat_map(|row| row.effects.iter().filter(|e| e.stop.is_none() && e.name == glow_name).map(|e| e.handle)).collect();
    assert!(glow_handles.iter().all(|h| r.s.rows.iter().any(|row| row.effects.iter().any(|e| e.stop.is_some() && e.handle == *h))), "every glow is stopped: {glow_handles:?}");
    r.idle(120);
    let alive: Vec<String> = r.s.fx.world.live().iter().map(|l| l.name.to_lowercase()).filter(|n| n == glow_name).collect();
    assert!(alive.is_empty(), "the glow is gone 2 s after the shot: {alive:?}");
    r.dummy_at(250.0);
    r.idle(30);
    // ---- FINISH_BZ
    r.finish_bz();
    let forms: &[&str] = if hunny { &["DW2", "BZ2"] } else { &["DW", "BZ"] };
    let (n, failed, own) = report(&mut r, log, forms);
    let rows = kh_core::combat::forms::arrowgun::FORM_ARROWGUN.iter().filter(|x| x.what == "wnotify" && forms.contains(&x.form)).count() + 1; // + the launcher's ChangeAttachParent
    assert_eq!(own, rows, "every own-clip row of {forms:?} judged");
    assert!(n > 0);
    assert!(failed.is_empty(), "{} of {n} notifies failed:\n{}", failed.len(), failed.join("\n"));
}

#[test]
fn shooting_star_double_arrowguns_to_magic_launcher_every_notify() {
    chain("WEP_KEYBLADE_SO_02", SHOOTING_STAR, "w_so020", (0x80, 0x83), ("StyleChange2", "StyleChange5"), "audit_a_arrowgun_ss");
}

#[test]
fn hunny_spout_hunny_blasters_to_hunny_launcher_every_notify() {
    chain("WEP_KEYBLADE_SO_07", HUNNY_SPOUT, "w_so040", (0x8E, 0x8F), ("StyleChange16", "StyleChange17"), "audit_a_arrowgun_hs");
}
